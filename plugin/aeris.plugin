import base64
import ipaddress
import json
import os
import socket
import struct
import threading

from base_plugin import BasePlugin
from ui.bulletin import BulletinHelper
from ui.settings import Divider, Header, Input, Text

__id__ = "aeris"
__name__ = "AERIS"
__description__ = "Send Telegram media to AERIS Desktop over your local network."
__author__ = "FlaiGame1"
__version__ = "0.1.0"
__icon__ = "exteraPlugins/1"
__app_version__ = ">=12.5.1"
__sdk_version__ = ">=1.4.4.3"
__category__ = ["media", "integrations"]

DEFAULT_HOST = "192.168.1.100"
DEFAULT_PORT = "45872"


def _read_exact(sock, count):
    data = bytearray()
    while len(data) < count:
        part = sock.recv(count - len(data))
        if not part:
            raise ConnectionError("Компьютер закрыл соединение")
        data.extend(part)
    return bytes(data)


def _send_ws_text(sock, text):
    payload = text.encode("utf-8")
    mask = os.urandom(4)
    length = len(payload)
    header = bytearray([0x81])
    if length < 126:
        header.append(0x80 | length)
    elif length < 65536:
        header.append(0x80 | 126)
        header.extend(struct.pack("!H", length))
    else:
        header.append(0x80 | 127)
        header.extend(struct.pack("!Q", length))
    masked = bytes(value ^ mask[index % 4] for index, value in enumerate(payload))
    sock.sendall(bytes(header) + mask + masked)


def _recv_ws_text(sock):
    first, second = _read_exact(sock, 2)
    opcode = first & 0x0F
    length = second & 0x7F
    if length == 126:
        length = struct.unpack("!H", _read_exact(sock, 2))[0]
    elif length == 127:
        length = struct.unpack("!Q", _read_exact(sock, 8))[0]
    if length > 65536:
        raise ValueError("Ответ AERIS слишком большой")
    mask = _read_exact(sock, 4) if second & 0x80 else None
    payload = _read_exact(sock, length)
    if mask:
        payload = bytes(value ^ mask[index % 4] for index, value in enumerate(payload))
    if opcode == 8:
        raise ConnectionError("Компьютер закрыл WebSocket")
    if opcode != 1:
        raise ValueError("Ожидался текстовый ответ AERIS")
    return json.loads(payload.decode("utf-8"))


def _exchange(sock, message):
    _send_ws_text(sock, json.dumps(message, separators=(",", ":")))
    return _recv_ws_text(sock)


def _connect_and_pair(host, port, code):
    sock = socket.create_connection((host, port), timeout=5)
    try:
        sock.settimeout(5)
        key = base64.b64encode(os.urandom(16)).decode("ascii")
        request = (
            "GET /ws HTTP/1.1\r\n"
            "Host: {}:{}\r\n"
            "Upgrade: websocket\r\n"
            "Connection: Upgrade\r\n"
            "Sec-WebSocket-Key: {}\r\n"
            "Sec-WebSocket-Version: 13\r\n\r\n"
        ).format(host, port, key)
        sock.sendall(request.encode("ascii"))
        response = b""
        while b"\r\n\r\n" not in response:
            chunk = sock.recv(1024)
            if not chunk:
                raise ConnectionError("Нет ответа WebSocket от компьютера")
            response += chunk
            if len(response) > 8192:
                raise ValueError("Некорректный ответ WebSocket")
        if b" 101 " not in response.split(b"\r\n", 1)[0]:
            raise ConnectionError("AERIS Desktop отклонил WebSocket")
        hello = _exchange(sock, {
            "protocol": "aeris", "version": 1, "type": "hello",
            "id": "hello-1", "payload": {}
        })
        if hello.get("type") != "hello_ack":
            raise ConnectionError("Неожиданный ответ AERIS")
        paired = _exchange(sock, {
            "protocol": "aeris", "version": 1, "type": "pair",
            "id": "pair-1", "payload": {"code": code}
        })
        if paired.get("type") != "pair_result" or not paired.get("payload", {}).get("paired"):
            reason = paired.get("payload", {}).get("reason", "pairing_failed")
            raise ConnectionError("Сопряжение не удалось: {}".format(reason))
        pong = _exchange(sock, {
            "protocol": "aeris", "version": 1, "type": "ping",
            "id": "ping-1", "payload": {}
        })
        if pong.get("type") != "pong":
            raise ConnectionError("Проверка связи AERIS не пройдена")
        return "AERIS подключён: {}:{}".format(host, port)
    finally:
        sock.close()


class AerisPlugin(BasePlugin):
    def on_plugin_load(self):
        self.logger.info("AERIS plugin loaded")

    def create_settings(self):
        return [
            Header(text="AERIS"),
            Text(
                text="Трансляция медиа из Telegram на компьютер",
                subtext="Подключите телефон и AERIS Desktop к одной сети Wi-Fi.",
                icon="msg2_devices",
            ),
            Divider(text="Подключение к компьютеру"),
            Input(
                key="host",
                text="IP-адрес компьютера",
                default=DEFAULT_HOST,
                subtext="Адрес, который показывает AERIS Desktop, например 192.168.1.50",
                icon="msg_text",
                on_change=self._save_host,
            ),
            Input(
                key="port",
                text="Порт",
                default=DEFAULT_PORT,
                subtext="Стандартный порт AERIS — 45872",
                icon="msg_settings",
                on_change=self._save_port,
            ),
            Divider(text="Сопряжение устройств"),
            Input(
                key="pairing_code",
                text="Код сопряжения",
                default="",
                subtext="Введите 6-значный код из окна AERIS Desktop.",
                icon="msg_secret",
                on_change=self._save_code,
            ),
            Text(
                text="Проверить подключение",
                subtext="Проверка связи и сопряжения с компьютером",
                icon="msg_link",
                accent=True,
                on_click=self._start_connection_test,
            ),
            Divider(),
            Text(
                text="AERIS 0.1.0 · Протокол v1",
                subtext="Тестовая сборка. Передавайте данные только в доверенной локальной сети.",
                icon="msg_info",
            ),
        ]

    def _save_host(self, value):
        self.set_setting("host", str(value).strip())

    def _save_port(self, value):
        self.set_setting("port", str(value).strip())

    def _save_code(self, value):
        self.set_setting("pairing_code", str(value).strip().upper())

    def _start_connection_test(self, _view=None):
        host = str(self.get_setting("host", DEFAULT_HOST)).strip()
        port_text = str(self.get_setting("port", DEFAULT_PORT)).strip()
        code = str(self.get_setting("pairing_code", "")).strip().upper()

        try:
            if not host or any(char.isspace() for char in host):
                raise ValueError("Укажите IP-адрес компьютера")
            try:
                ipaddress.ip_address(host)
            except ValueError:
                if len(host) > 253 or any(not part or len(part) > 63 for part in host.split(".")):
                    raise ValueError("Проверьте IP-адрес или имя компьютера")
            port = int(port_text)
            if not 1 <= port <= 65535:
                raise ValueError("Порт должен быть от 1 до 65535")
            if len(code) != 6 or not code.isalnum():
                raise ValueError("Введите 6-значный код сопряжения")
        except Exception as error:
            BulletinHelper.show_error(str(error))
            return

        BulletinHelper.show_info("Подключаемся к AERIS Desktop…")

        def worker():
            try:
                result = _connect_and_pair(host, port, code)
                self.logger.info(result)
                BulletinHelper.show_success(result)
            except Exception as error:
                self.logger.error("AERIS connection failed: {}".format(error))
                BulletinHelper.show_error("Не удалось подключиться: {}".format(error))

        threading.Thread(target=worker, name="AERIS-connect-test", daemon=True).start()
