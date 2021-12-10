import os
import yaml
import subprocess

from typing import List, Callable, Optional
from contextlib import contextmanager

from persia.error import FileNotFoundException
from persia.logger import get_default_logger
from persia.env import PERSIA_LAUNCHER_VERBOSE

_logger = get_default_logger()


def setup_seed(seed: int):
    """Set the random seed for dependencies to ensure that experiments are reproducible.

    Arguments:
        seed (int): integer to use as seed for random numebr generator used by random, NumPy and pyTorch.
    """
    import numpy as np
    import torch
    import random

    np.random.seed(seed)

    random.seed(seed)

    torch.random.manual_seed(seed)
    if getattr(torch, "use_deterministic_algorithms", None):
        torch.use_deterministic_algorithms(True)
    else:
        torch.backends.cudnn.deterministic = True


def load_yaml(filepath: str) -> dict:
    """Load the yaml config by provided filepath

    Arguments:
        filepath (str): yaml config path
    """
    if not os.path.exists(filepath):
        raise FileNotFoundException(f"filepath {filepath} not found!")

    with open(filepath, "r") as file:
        return yaml.load(file, Loader=yaml.FullLoader)


def run_command(cmd: List[str], env: os._Environ = None):
    cmd = list(map(str, cmd))
    if PERSIA_LAUNCHER_VERBOSE:
        cmd_str = " ".join(cmd)
        _logger.info(f"execute command: {cmd_str}")

    subprocess.check_call(cmd, env=env)