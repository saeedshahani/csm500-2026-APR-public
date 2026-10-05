#!/usr/bin/env python3
"""Project entry point for organization evidence verification and reproduction."""
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from tests.oracles.organization.study import main
if __name__ == '__main__': main()
