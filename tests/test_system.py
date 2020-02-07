import pathlib, pytest, subprocess
from unittest import mock

from mopidy_iris.system import IrisSystemThread, IrisSystemPermissionError


def test_system_sh_path():
    iris_system = IrisSystemThread("foo", None, None)
    assert iris_system.script_path.is_file()
    assert iris_system.script_path.name == "system.sh"


def test_can_run():
    iris_system = IrisSystemThread("foo", None, None)
    iris_system._USE_SUDO = False
    assert iris_system.can_run() is True


@pytest.fixture
def popen_mock():
    patcher = mock.patch("subprocess.Popen", spec=True)
    yield patcher.start()
    patcher.stop()


@pytest.fixture
def process_mock(popen_mock):
    mock_process = popen_mock.return_value
    mock_process.communicate.return_value = ("", None)
    mock_process.wait.return_value = 0
    yield mock_process


def test_can_run_args(popen_mock, process_mock):
    IrisSystemThread("foo", None, None).can_run()
    popen_mock.assert_called_once_with(
        mock.ANY, shell=True, stderr=subprocess.PIPE, stdout=subprocess.PIPE
    )


def test_can_run_uses_sudo_non_interactive(popen_mock, process_mock):
    IrisSystemThread("foo", None, None).can_run()

    popen_mock.assert_called_once()
    assert popen_mock.call_args[0][0].startswith(b"sudo -n ")


def test_can_run_calls_script_check(popen_mock, process_mock):
    IrisSystemThread("foo", None, None).can_run()

    assert popen_mock.call_args[0][0].endswith(b"system.sh check")


def test_can_run_sudo_refused_raises(popen_mock, process_mock, caplog):
    process_mock.wait.return_value = 1
    iris_system = IrisSystemThread("foo", None, None)

    with pytest.raises(IrisSystemPermissionError) as excinfo:
        iris_system.can_run()

    error_message = (
        "Password-less access to %s was refused. "
        "Check your /etc/sudoers file." % iris_system.script_path.as_uri()
    )
    assert error_message in str(excinfo.value)
    assert error_message in caplog.text


@pytest.mark.skip(reason="todo")
def test_run_args(popen_mock, process_mock):
    iris_system = IrisSystemThread("foo", mock.Mock(), None)
    iris_system.can_run = mock.Mock(return_value=True)
    iris_system.run()

    popen_mock.assert_called_once_with(
        mock.ANY, stderr=subprocess.PIPE, stdout=subprocess.PIPE
    )


@pytest.mark.skip(reason="todo")
def test_run_uses_sudo(popen_mock, process_mock):
    iris_system = IrisSystemThread("foo", mock.Mock(), None)
    iris_system.can_run = mock.Mock(return_value=True)
    iris_system.run()

    popen_mock.assert_called_once()
    assert popen_mock.call_args[0][0][0] == b"sudo"
