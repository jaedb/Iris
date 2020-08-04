import pytest
from asyncio import Future
from unittest import mock

import tornado.testing
import tornado.web
from tornado.httpclient import HTTPResponse
from tornado.escape import json_decode

from mopidy_iris import handlers
from mopidy_iris import core
from mopidy_iris.mem import iris


def async_return_helper(result):
    f = Future()
    f.set_result(result)
    return f


class HttpHandlerTest(tornado.testing.AsyncHTTPTestCase):
    @pytest.fixture(autouse=True)
    def inject_fixtures(self, caplog):
        self._caplog = caplog

    def get_app(self):
        http_handler = handlers.HttpHandler
        # http_handler.handle_result = mock.Mock()
        # self.handler_mock = http_handler.handle_result
        return tornado.web.Application(
            [(r"/(.*)", http_handler, {"core": None, "config": {},},)]
        )

    def test_get_method(self):
        with mock.patch("time.time", return_value=100):
            response = self.fetch("/test", method="GET")

        assert 200 == response.code
        result = json_decode(response.body)
        assert "2.0" == result["jsonrpc"]
        assert "test" == result["method"]
        assert 100 == result["id"]
        assert "Running test... please wait" == result["result"]["message"]

    def test_get_method_headers(self):
        response = self.fetch("/test", method="GET")

        assert response.headers["Access-Control-Allow-Origin"] == "*"
        assert "Origin" in response.headers["Access-Control-Allow-Headers"]

    def test_get_unknown_method_is_error(self):
        response = self.fetch("/baz", method="GET")

        assert 400 == response.code
        error = json_decode(response.body)["error"]
        assert "Method baz does not exist" == error["message"]

    @mock.patch.object(handlers, "iris")
    def test_get_method_called(self, iris_mock):
        iris_mock.foo = mock.Mock()

        response = self.fetch("/foo", method="GET")

        iris_mock.foo.assert_called_once()
        assert 200 == response.code

    @mock.patch.object(handlers, "iris")
    def test_get_method_any_exception_handled(self, iris_mock):
        iris_mock.foo = mock.Mock(side_effect=Exception("bar"))

        response = self.fetch("/foo", method="GET")

        iris_mock.foo.assert_called_once()
        assert 200 == response.code
        assert "bar" in self._caplog.text

    @mock.patch.object(handlers.iris, "do_fetch")
    def test_get_method_with_fetch(self, fetch_mock):
        iris.config = {"spotify": {"client_id": 123, "client_secret": 456}}
        result = mock.Mock(spec=HTTPResponse, body='{"expires_in":88}')
        fetch_mock.return_value = async_return_helper(result)

        response = self.fetch("/refresh_spotify_token", method="GET")

        assert 200 == response.code
        assert len(response.body) > 0
        result = json_decode(response.body)
        assert "refresh_spotify_token" == result["method"]
        assert 88 == result["result"]["spotify_token"]["expires_in"]
