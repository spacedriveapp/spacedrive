# File: .scripts/validate_tasks.py
import sys
import subprocess
import yaml
import json
from jsonschema import validate, ValidationError

# Load the schema
with open('.tasks/task.schema.json', 'r') as f:
    schema = json.load(f)

# Get a list of staged markdown files in the .tasks/ directory
staged_files_proc = subprocess.run(
    ['git', 'diff', '--cached', '--name-only', '--diff-filter=ACM', '--', '.tasks/*.md'],
    capture_output=True, text=True
)
staged_files = staged_files_proc.stdout.strip().split('\n')

has_errors = False

for file_path in staged_files:
    if not file_path:
        continue

    try:
        with open(file_path, 'r') as f:
            content = f.read()
            # Split out the front matter
            if content.startswith('---'):
                front_matter_str = content.split('---')[1]
                front_matter = yaml.safe_load(front_matter_str)
                
                # Validate against the schema
                validate(instance=front_matter, schema=schema)
                print(f"✅ Validated: {file_path}")
            else:
                print(f"⚠️  Skipped (no front matter): {file_path}")

    except (ValidationError, yaml.YAMLError) as e:
        print(f"❌ ERROR in {file_path}:\n   {e}\n", file=sys.stderr)
        has_errors = True
    except FileNotFoundError:
        # This can happen if a file was staged for deletion
        continue

# If there were any errors, exit with a non-zero status code to block the commit
if has_errors:
    print("\nCommit aborted due to validation errors in task files.", file=sys.stderr)
    sys.exit(1)

sys.exit(0)
