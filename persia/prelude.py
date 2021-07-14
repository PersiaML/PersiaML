import sys

from types import ModuleType

# pytype: disable=import-error
import persia_core

# pytype: enable=import-error


def register_submodule(module: ModuleType, root_module_path: str):
    """register the persia py client wrapper module to sys module

    Arguments:
        module (ModuleType): root module
        root_module_path (str): root module path
    """
    for attr in dir(module):
        if attr.startswith("__"):
            continue
        obj = getattr(module, attr)
        if isinstance(obj, ModuleType):
            submodule_name = attr
            full_path = f"{root_module_path}.{submodule_name}"
            sys.modules[full_path] = obj
            register_submodule(obj, full_path)


register_submodule(
    persia_core,
    persia_core.__name__,
)


# pytype: disable=import-error
from persia_core import is_cuda_feature_available

from persia_core import (
    PyPersiaRpcClient,
)  # flake8: noqa
from persia_core.optim import PyOptimizerBase  # flake8: noqa
from persia_core.data import (
    PyPersiaBatchData,
    PyPersiaBatchDataChannel,
    PyPersiaBatchDataSender,
    PyPersiaBatchDataReceiver,
)  # flake8: noqa
from persia_core.utils import (
    PyPersiaMessageQueueServer,
    PyPersiaMessageQueueClient,
    PyPersiaReplicaInfo,
)  # flake8: noqa
from persia_core.nats import (
    PyPersiaBatchFlowNatsStubPublisher,
    PyPersiaBatchFlowNatsStubResponder,
)  # flake8: noqa

if is_cuda_feature_available():
    from persia_core.backward import PyBackward  # flake8: noqa
    from persia_core.forward import PyForward  # flake8: noqa

# pytype: enable=import-error
