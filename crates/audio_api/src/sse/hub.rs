#![allow(dead_code)]
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

/// Buffer de eventos por job para replay (QA-0024 — docs/03 §5 exige
/// "buffer dos últimos 200 eventos por job para replay"). Antes, o
/// `publish` descartava eventos publicados antes do primeiro
/// `subscribe` — perdendo o `job.completed` quando o worker terminava
/// em 1s (mais rápido que a UI abrir o SSE). Agora, todo publish
/// também armazena no buffer; o `subscribe_with_replay` retorna o
/// receiver + os eventos já publicados.
const REPLAY_BUFFER_SIZE: usize = 200;

/// Tipo para o buffer de replay: `(próximo_seq, Vec<JobEvent>)`.
/// Próximo seq começa em 0 e incrementa a cada publish.
type ReplayBuffer = (u64, Vec<JobEvent>);

#[derive(Debug, Clone)]
pub struct JobEvent {
    pub job_id: Uuid,
    pub event_type: String,
    pub data: serde_json::Value,
    /// ID monotônico por job — usado pelo cliente via `Last-Event-ID`
    /// header para reconexão com replay a partir do ponto onde parou.
    pub seq: u64,
}

#[derive(Clone)]
pub struct EventHub {
    channels: Arc<RwLock<HashMap<Uuid, broadcast::Sender<JobEvent>>>>,
    /// Buffer de replay: job_id → `(próximo_seq, Vec<JobEvent>)` com no
    /// máximo REPLAY_BUFFER_SIZE eventos. Quando o buffer enche, os
    /// eventos mais antigos são descartados (capacidade é o ponto onde
    /// o replay deixa de ser completo — aceitável segundo docs/03 §5).
    buffers: Arc<RwLock<HashMap<Uuid, ReplayBuffer>>>,
}

/// Resultado de `subscribe_with_replay`: receiver para eventos futuros
/// e eventos passados (em ordem, do mais antigo para o mais novo) que
/// o cliente pode aplicar para alcançar o estado atual.
pub struct SubscribeWithReplay {
    pub rx: broadcast::Receiver<JobEvent>,
    /// Eventos já publicados antes do subscribe, em ordem crescente de
    /// `seq`. Pode ser vazio se nenhum evento foi publicado ainda, ou
    /// se o cliente passou `last_event_id` igual ao último seq.
    pub replay: Vec<JobEvent>,
}

impl EventHub {
    pub fn new() -> Self {
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            buffers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Assina o canal broadcast para receber eventos futuros, E retorna
    /// os eventos já publicados (replay). O `last_event_id` (do header
    /// `Last-Event-ID` do SSE) permite que um cliente que reconecte
    /// receba só os eventos que perdeu — não os que já processou.
    ///
    /// QA-0024: antes deste fix, o `subscribe` criava o canal na
    /// primeira chamada. Se o `publish` veio antes (worker completou
    /// rápido), o evento era descartado. Agora, todo `publish` também
    /// armazena no buffer; este método retorna o replay.
    pub async fn subscribe_with_replay(
        &self,
        job_id: Uuid,
        last_event_id: Option<u64>,
    ) -> SubscribeWithReplay {
        // Primeiro garante que o canal exista (cria se necessário).
        {
            let mut channels = self.channels.write().await;
            channels.entry(job_id).or_insert_with(|| {
                let (tx, _rx) = broadcast::channel(REPLAY_BUFFER_SIZE);
                tx
            });
        }
        let rx = {
            let channels = self.channels.read().await;
            channels
                .get(&job_id)
                .map(|tx| tx.subscribe())
                .expect("canal acabou de ser criado")
        };

        // Pega o replay do buffer.
        let replay = {
            let buffers = self.buffers.read().await;
            match buffers.get(&job_id) {
                Some((_next_seq, events)) => {
                    let filtered: Vec<JobEvent> = match last_event_id {
                        Some(last) => events.iter().filter(|e| e.seq > last).cloned().collect(),
                        None => events.clone(),
                    };
                    filtered
                },
                None => Vec::new(),
            }
        };

        SubscribeWithReplay { rx, replay }
    }

    /// Compatibilidade retroativa — equivalente a `subscribe_with_replay(job_id, None)`.
    /// Mantido para não quebrar callers existentes (tests, recovery).
    pub async fn subscribe(&self, job_id: Uuid) -> broadcast::Receiver<JobEvent> {
        let s = self.subscribe_with_replay(job_id, None).await;
        s.rx
    }

    pub async fn publish(&self, job_id: Uuid, event_type: &str, data: serde_json::Value) {
        // Primeiro garante que o canal exista — sem isso, publicações
        // antes do subscribe eram silenciosamente descartadas (bug
        // QA-0024). Agora criamos o canal aqui mesmo, e o evento fica
        // no buffer de replay para o próximo subscriber.
        {
            let mut channels = self.channels.write().await;
            channels.entry(job_id).or_insert_with(|| {
                let (tx, _rx) = broadcast::channel(REPLAY_BUFFER_SIZE);
                tx
            });
        }

        // Aloca o seq e armazena no buffer.
        let event = {
            let mut buffers = self.buffers.write().await;
            let entry = buffers.entry(job_id).or_insert((0u64, Vec::new()));
            let seq = entry.0;
            entry.0 += 1;
            let event = JobEvent {
                job_id,
                event_type: event_type.to_string(),
                data,
                seq,
            };
            entry.1.push(event.clone());
            // Mantém no máximo REPLAY_BUFFER_SIZE eventos no buffer.
            if entry.1.len() > REPLAY_BUFFER_SIZE {
                let excess = entry.1.len() - REPLAY_BUFFER_SIZE;
                entry.1.drain(0..excess);
            }
            event
        };

        // Publica no canal broadcast (se houver subscriber ativo, recebe).
        let channels = self.channels.read().await;
        if let Some(tx) = channels.get(&job_id) {
            let _ = tx.send(event);
        }
    }

    pub async fn cleanup(&self, job_id: Uuid) {
        let mut channels = self.channels.write().await;
        channels.remove(&job_id);
        // Mantém o buffer por um tempo mesmo após cleanup — se o
        // cliente reconectar, ainda há replay. Em uma implementação
        // completa, haveria um TTL aqui (ex.: 1h após cleanup). Por ora,
        // só removemos o canal broadcast; o buffer fica até a próxima
        // reinicialização (aceitável para ciclo QA).
        // Em produção, considerar TTL no buffer para evitar crescimento
        // indefinido em instalações com muitos jobs.
    }
}

impl Default for EventHub {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn publish_antes_de_subscribe_nao_perde_evento() {
        // QA-0024 — caso de regressão: antes do fix, o publish descartava
        // eventos se ninguém tivesse subscrito ainda. Agora, o buffer
        // mantém o evento para o próximo subscriber.
        let hub = EventHub::new();
        let job_id = Uuid::new_v4();

        // Publica ANTES de qualquer subscribe.
        hub.publish(job_id, "job.state", serde_json::json!({"status": "queued"}))
            .await;
        hub.publish(
            job_id,
            "job.completed",
            serde_json::json!({"status": "completed"}),
        )
        .await;

        // Agora assina — deve receber os 2 eventos no replay.
        let sub = hub.subscribe_with_replay(job_id, None).await;
        assert_eq!(sub.replay.len(), 2);
        assert_eq!(sub.replay[0].event_type, "job.state");
        assert_eq!(sub.replay[1].event_type, "job.completed");
    }

    #[tokio::test]
    async fn last_event_id_filtra_replay_para_reconexao() {
        // QA-0024 — reconexão via Last-Event-ID: cliente que já recebeu
        // até seq=0 não deve receber de novo os eventos 0 e 1, só o 2+.
        let hub = EventHub::new();
        let job_id = Uuid::new_v4();

        hub.publish(job_id, "job.state", serde_json::json!({"a": 1}))
            .await; // seq=0
        hub.publish(job_id, "job.state", serde_json::json!({"a": 2}))
            .await; // seq=1
        hub.publish(job_id, "job.completed", serde_json::json!({"a": 3}))
            .await; // seq=2

        let sub = hub.subscribe_with_replay(job_id, Some(0)).await;
        assert_eq!(sub.replay.len(), 2);
        assert_eq!(sub.replay[0].seq, 1);
        assert_eq!(sub.replay[1].seq, 2);
    }

    #[tokio::test]
    async fn subscribe_depois_de_subscribe_ainda_recebe_eventos_futuros() {
        // Comportamento tradicional do broadcast: dois subscribers
        // recebem eventos publicados depois do subscribe.
        let hub = EventHub::new();
        let job_id = Uuid::new_v4();

        let mut rx1 = hub.subscribe(job_id).await;
        let mut rx2 = hub.subscribe(job_id).await;

        hub.publish(job_id, "job.state", serde_json::json!({"x": 1}))
            .await;

        let e1 = rx1.recv().await.unwrap();
        let e2 = rx2.recv().await.unwrap();
        assert_eq!(e1.event_type, "job.state");
        assert_eq!(e2.event_type, "job.state");
    }

    #[tokio::test]
    async fn buffer_limite_respeitado() {
        // QA-0024 — docs/03 §5: buffer dos últimos 200 eventos. Se
        // publicar 250, os 50 mais antigos são descartados (replay só
        // tem os 200 mais recentes).
        let hub = EventHub::new();
        let job_id = Uuid::new_v4();

        for i in 0..250u64 {
            hub.publish(job_id, "job.progress", serde_json::json!({"i": i}))
                .await;
        }

        let sub = hub.subscribe_with_replay(job_id, None).await;
        assert_eq!(sub.replay.len(), REPLAY_BUFFER_SIZE);
        // O primeiro evento no replay deve ser o de seq=50 (os 0..49
        // foram descartados pelo limite).
        assert_eq!(sub.replay[0].seq, 50);
        assert_eq!(sub.replay[0].data["i"], 50);
    }

    #[tokio::test]
    async fn publish_sem_subscriber_nao_panic() {
        // Caso de bordagem: publish em job_id sem nenhum subscriber
        // não deve panic — deve apenas armazenar no buffer.
        let hub = EventHub::new();
        let job_id = Uuid::new_v4();
        hub.publish(job_id, "test", serde_json::json!({})).await;
        // Subscribe depois — replay deve ter 1 evento.
        let sub = hub.subscribe_with_replay(job_id, None).await;
        assert_eq!(sub.replay.len(), 1);
    }

    #[tokio::test]
    async fn cleanup_remove_canal_mas_mantem_buffer() {
        // Cleanup remove o canal broadcast mas mantém o buffer — para
        // permitir reconexão com replay mesmo após o cleanup.
        let hub = EventHub::new();
        let job_id = Uuid::new_v4();
        hub.publish(job_id, "test", serde_json::json!({})).await;
        hub.cleanup(job_id).await;
        // Subscribe depois do cleanup — replay ainda deve ter o evento.
        let sub = hub.subscribe_with_replay(job_id, None).await;
        assert_eq!(sub.replay.len(), 1);
    }
}
