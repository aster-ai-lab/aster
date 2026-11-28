"""Compare account usage when the user requires a fresh source check on both turns."""
from hashlib import sha256
from importlib.util import module_from_spec, spec_from_file_location
import json
from pathlib import Path

benchmark = Path(__file__).resolve().parent.parent / '2026-11-27-deepseek-harness-token-comparison' / 'compare.py'
spec = spec_from_file_location('frozen_fixture', benchmark)
driver = module_from_spec(spec)
spec.loader.exec_module(driver)
values = [{'slot': slot, 'value': 'C-' + sha256(f'catalog:{slot}'.encode()).hexdigest()[:16]}
          for slot in range(96)]
driver.COMMAND = "cat inventory.json"
driver.FILES = {'inventory.json': json.dumps({'items': values}, separators=(',', ':')).encode() + b'\n'}
driver.TASK = (
    ('Run the shell command `cat inventory.json` exactly. Quote the value for slot 17 verbatim.', (values[17]['value'],)),
    ('Independently verify the current file by running the same shell command `cat inventory.json` again exactly. Quote the value for slot 77 verbatim.', (values[77]['value'],)),
)
driver.main()
