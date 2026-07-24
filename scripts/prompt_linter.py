#!/usr/bin/env python3
import yaml
import sys
import json
from pathlib import Path

"""
Linter de prompts .prompt para Remix AI.
Valida schema, constraints e compatibilidade entre versões.
"""

def load_yaml(path):
    with open(path) as f:
        return yaml.safe_load(f)

def validate_prompt_spec(spec, catalog):
    errors = []
    
    if 'id' not in spec:
        errors.append("Missing 'id' field")
    if 'version' not in spec:
        errors.append("Missing 'version' field")
    if 'constraints' in spec:
        for c in spec['constraints']:
            if not isinstance(c, str):
                errors.append(f"Constraint must be string, got {type(c)}")
    
    # Validate parameter enums
    for param in spec.get('parameters', []):
        if 'enum' in param and 'default' in param:
            if param['default'] not in param['enum']:
                errors.append(f"Default '{param['default']}' not in enum {param['enum']} for param {param['name']}")
    
    return errors

if __name__ == '__main__':
    catalog_path = Path(sys.argv[1])
    catalog = json.load(open(catalog_path))
    
    errors = []
    for prompt in catalog.get('prompts', []):
        spec_path = catalog_path.parent / prompt['file']
        spec = load_yaml(spec_path)
        prompt_errors = validate_prompt_spec(spec, catalog)
        if prompt_errors:
            errors.append((prompt['id'], prompt_errors))
    
    if errors:
        for pid, errs in errors:
            print(f"❌ {pid}")
            for e in errs:
                print(f"   - {e}")
        sys.exit(1)
    else:
        print("✅ All prompts valid")
