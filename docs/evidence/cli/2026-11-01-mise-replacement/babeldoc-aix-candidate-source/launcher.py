from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent / "site-packages"))
sys.argv[0] = "babeldoc-stream"
from babeldoc.main import cli

if __name__ == "__main__":
    cli()
