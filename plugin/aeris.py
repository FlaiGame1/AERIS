import base64, hashlib, ipaddress, json, mimetypes, os, socket, struct, threading
from base_plugin import BasePlugin, MenuItemData, MenuItemType
from ui.bulletin import BulletinHelper
from ui.settings import Divider, Header, Input, Text

__id__ = "aeris"
__name__ = "AERIS"
__description__ = "Cast Telegram media to AERIS Desktop over your local network."
__author__ = "FlaiGame1"
__version__ = "0.3.0"
__icon__ = "exteraPlugins/1"
__app_version__ = ">=12.5.1"
__sdk_version__ = ">=1.4.4.3"
__category__ = ["media", "integrations"]

DEFAULT_HOST = "192.168.1.100"
DEFAULT_PORT = "45872"
CHUNK_SIZE = 1024 * 1024
MAX_MEDIA_BYTES = 512 * 1024 * 1024


def _read_exact(sock, count):
    data = bytearray()
    while len(data) < count:
        part = sock.recv(count - len(data))
        if not part:
            raise ConnectionError("Компьютер закрыл соединение")
        data.extend(part)
    return bytes(data)


def _send_frame(sock, payload, opcode=1):
    mask = os.urandom(4)
    size = len(payload)
    head = bytearray([0x80 | opcode])
    if size < 126:
        head.append(0x80 | size)
    elif size < 65536:
        head.append(0x80 | 126)
        head.extend(struct.pack("!H", size))
    else:
        head.append(0x80 | 127)
        head.extend(struct.pack("!Q", size))
    masked = bytes(value ^ mask[index % 4] for index, value in enumerate(payload))
    sock.sendall(bytes(head) + mask + masked)


def _recv_text(sock):
    while True:
        first, second = _read_exact(sock, 2)
        opcode = first & 15
        size = second & 127
        if size == 126:
            size = struct.unpack("!H", _read_exact(sock, 2))[0]
        elif size == 127:
            size = struct.unpack("!Q", _read_exact(sock, 8))[0]
        if size > 65536:
            raise ValueError("Ответ AERIS слишком большой")
        mask = _read_exact(sock, 4) if second & 128 else None
        data = _read_exact(sock, size)
        if mask:
            data = bytes(value ^ mask[index % 4] for index, value in enumerate(data))
        if opcode == 8:
            raise ConnectionError("Компьютер закрыл WebSocket")
        if opcode == 9:
            _send_frame(sock, data, 10)
            continue
        if opcode == 1:
            return json.loads(data.decode("utf-8"))


def _close_ws(sock):
    """Perform a normal WebSocket close handshake instead of resetting TCP."""
    try:
        _send_frame(sock, struct.pack("!H", 1000), 8)
        sock.settimeout(2)
        first, second = _read_exact(sock, 2)
        size = second & 127
        if size == 126:
            size = struct.unpack("!H", _read_exact(sock, 2))[0]
        elif size == 127:
            size = struct.unpack("!Q", _read_exact(sock, 8))[0]
        if second & 128:
            _read_exact(sock, 4)
        if size:
            _read_exact(sock, min(size, 125))
    except Exception:
        pass
    finally:
        try:
            sock.close()
        except Exception:
            pass


def _send_msg(sock, kind, payload=None, req="aeris"):
    _send_frame(sock, json.dumps({
        "protocol": "aeris", "version": 1, "type": kind,
        "id": req, "payload": payload or {}
    }, separators=(",", ":")).encode("utf-8"), 1)
    response = _recv_text(sock)
    if response.get("type") == "error":
        details = response.get("payload", {})
        raise RuntimeError("{}: {}".format(details.get("code"), details.get("message", "")))
    return response


def _connect(host, port, code):
    sock = socket.create_connection((host, port), timeout=10)
    sock.settimeout(30)
    key = base64.b64encode(os.urandom(16)).decode()
    request = (
        "GET /ws HTTP/1.1\r\n"
        "Host: {}:{}\r\n"
        "Upgrade: websocket\r\n"
        "Connection: Upgrade\r\n"
        "Sec-WebSocket-Key: {}\r\n"
        "Sec-WebSocket-Version: 13\r\n\r\n"
    ).format(host, port, key)
    try:
        sock.sendall(request.encode())
        response = b""
        while b"\r\n\r\n" not in response:
            part = sock.recv(1024)
            if not part:
                raise ConnectionError("Нет ответа WebSocket")
            response += part
            if len(response) > 8192:
                raise ValueError("Некорректный ответ WebSocket")
        if b" 101 " not in response.split(b"\r\n", 1)[0]:
            raise ConnectionError("AERIS отклонил WebSocket")
        if _send_msg(sock, "hello").get("type") != "hello_ack":
            raise ConnectionError("Неожиданный ответ AERIS")
        paired = _send_msg(sock, "pair", {"code": code})
        if paired.get("type") != "pair_result" or not paired.get("payload", {}).get("paired"):
            raise ConnectionError("Неверный или истёкший код сопряжения")
        return sock
    except Exception:
        sock.close()
        raise


def _media_path(message, account=None):
    from client_utils import get_file_loader
    owner = getattr(message, "messageOwner", None)
    loader = get_file_loader(account) if account is not None else get_file_loader()
    for item in (message, owner):
        if item is None:
            continue
        try:
            file_obj = loader.getPathToMessage(item)
            path = str(file_obj.getAbsolutePath())
            if os.path.isfile(path):
                return path
        except Exception:
            pass
    raise FileNotFoundError(
        "Файл не найден в кэше Telegram. Сначала полностью скачай аудио или видео в Telegram."
    )


def _transfer(host, port, code, path):
    size = os.path.getsize(path)
    if size <= 0 or size > MAX_MEDIA_BYTES:
        raise ValueError("Файл пустой или больше лимита 512 МиБ")
    digest = hashlib.sha256()
    with open(path, "rb") as source:
        while True:
            chunk = source.read(CHUNK_SIZE)
            if not chunk:
                break
            digest.update(chunk)

    sock = _connect(host, port, code)
    try:
        mime_type = mimetypes.guess_type(path)[0] or "application/octet-stream"
        ready = _send_msg(sock, "begin_media", {
            "file_name": os.path.basename(path),
            "mime_type": mime_type,
            "size": size,
            "sha256": digest.hexdigest()
        }, "begin")
        if ready.get("type") != "media_ready":
            raise RuntimeError("ПК не подготовил приём файла")

        sent = 0
        with open(path, "rb") as source:
            while True:
                chunk = source.read(CHUNK_SIZE)
                if not chunk:
                    break
                _send_frame(sock, chunk, 2)
                sent += len(chunk)

        completed = _send_msg(sock, "finish_media", {}, "finish")
        if completed.get("type") != "media_complete":
            raise RuntimeError("ПК не подтвердил получение файла")
        playback = completed.get("payload", {}).get("playback", "")
        if playback == "playing_in_aeris":
            return "Воспроизведение запущено на ПК: {} ({:.1f} МБ)".format(
                os.path.basename(path), sent / 1048576
            )
        if playback == "opened_external_player":
            return "Видео открыто в системном плеере: {}".format(os.path.basename(path))
        if playback == "audio_playback_failed":
            raise RuntimeError("Файл принят, но AERIS не смог воспроизвести аудио. Проверь формат и устройство вывода Windows.")
        return "Файл передан на ПК: {} ({:.1f} МБ)".format(
            os.path.basename(path), sent / 1048576
        )
    finally:
        _close_ws(sock)


def _send_control(host, port, code, command, payload=None):
    sock = _connect(host, port, code)
    try:
        response = _send_msg(sock, command, payload or {}, "control-" + command)
        if response.get("type") != "command_result" or not response.get("payload", {}).get("success"):
            raise RuntimeError("ПК не подтвердил выполнение команды")
        return True
    finally:
        _close_ws(sock)


class AerisPlugin(BasePlugin):
    def on_plugin_load(self):
        self.logger.info("AERIS 0.3.0 loaded")
        self.menu_ids = []
        self.menu_ids.append(self.add_menu_item(MenuItemData(
            menu_type=MenuItemType.MESSAGE_CONTEXT_MENU,
            text="Передать через AERIS",
            on_click=self._on_message_menu,
            icon="msg2_devices",
            subtext="Воспроизвести медиа на компьютере",
            priority=100
        )))
        for command, title, icon in (
            ("play", "AERIS: продолжить", "msg_play"),
            ("pause", "AERIS: пауза", "msg_pause"),
            ("stop", "AERIS: остановить", "msg_close"),
        ):
            self.menu_ids.append(self.add_menu_item(MenuItemData(
                menu_type=MenuItemType.MESSAGE_CONTEXT_MENU,
                text=title,
                on_click=self._control_handler(command),
                icon=icon,
                priority=101
            )))

    def on_plugin_unload(self):
        for item_id in getattr(self, "menu_ids", []):
            try:
                self.remove_menu_item(item_id)
            except Exception:
                pass

    def create_settings(self):
        return [
            Header(text="AERIS"),
            Text(text="Передача Telegram-медиа на компьютер и удалённое управление воспроизведением.",
                 subtext="Передача идёт напрямую по локальной сети. Telegram-серверы не участвуют в соединении AERIS.",
                 icon="msg2_devices"),
            Divider(text="Подключение к компьютеру"),
            Input(key="host", text="IP-адрес компьютера", default=DEFAULT_HOST,
                  subtext="Адрес из окна AERIS Desktop.", icon="msg_text", on_change=self._save_host),
            Input(key="port", text="Порт", default=DEFAULT_PORT,
                  subtext="Стандартный порт — 45872.", icon="msg_settings", on_change=self._save_port),
            Input(key="pairing_code", text="Код сопряжения", default="",
                  subtext="Введите 6-значный код из AERIS Desktop.", icon="msg_secret", on_change=self._save_code),
            Text(text="Проверить подключение", subtext="Проверка локальной связи и сопряжения.",
                 icon="msg_link", accent=True, on_click=self._test),
            Divider(text="Управление воспроизведением"),
            Text(text="▶ Продолжить", subtext="Возобновить воспроизведение на ПК.",
                 icon="msg_play", on_click=self._control_from_settings("play")),
            Text(text="Ⅱ Пауза", subtext="Приостановить воспроизведение.",
                 icon="msg_pause", on_click=self._control_from_settings("pause")),
            Text(text="■ Остановить", subtext="Остановить текущий трек.",
                 icon="msg_close", on_click=self._control_from_settings("stop")),
            Input(key="seek_seconds", text="Перемотка, секунды", default="0",
                  subtext="Укажи позицию и нажми «Перемотать».", icon="msg_time", on_change=self._save_seek),
            Text(text="Перемотать", icon="msg_forward", accent=True, on_click=self._seek),
            Input(key="volume", text="Громкость (0–100)", default="80",
                  subtext="Громкость встроенного аудиоплеера AERIS.", icon="msg_settings", on_change=self._save_volume),
            Text(text="Применить громкость", icon="msg_settings", accent=True, on_click=self._set_volume),
            Divider(),
            Text(text="AERIS 0.3.0 · LAN control",
                 subtext="Аудио играет внутри AERIS. Видео открывается в системном плеере. Медиа должно быть загружено в кэш Telegram.",
                 icon="msg_info")
        ]

    def _save_host(self, value):
        self.set_setting("host", str(value).strip())

    def _save_port(self, value):
        self.set_setting("port", str(value).strip())

    def _save_code(self, value):
        self.set_setting("pairing_code", str(value).strip().upper())

    def _save_seek(self, value):
        self.set_setting("seek_seconds", str(value).strip())

    def _save_volume(self, value):
        self.set_setting("volume", str(value).strip())

    def _values(self):
        host = str(self.get_setting("host", DEFAULT_HOST)).strip()
        port = int(str(self.get_setting("port", DEFAULT_PORT)).strip())
        code = str(self.get_setting("pairing_code", "")).strip().upper()
        if not host or any(char.isspace() for char in host):
            raise ValueError("Укажи IP-адрес компьютера")
        try:
            ipaddress.ip_address(host)
        except ValueError:
            if len(host) > 253 or any(not part or len(part) > 63 for part in host.split(".")):
                raise ValueError("Проверь IP-адрес")
        if not 1 <= port <= 65535:
            raise ValueError("Неверный порт")
        if len(code) != 6 or not code.isalnum():
            raise ValueError("Введи 6-значный код сопряжения")
        return host, port, code

    def _run_control(self, command, payload=None):
        try:
            host, port, code = self._values()
            _send_control(host, port, code, command, payload)
            BulletinHelper.show_success("AERIS: команда выполнена")
        except Exception as error:
            self.logger.exception("AERIS control failed")
            BulletinHelper.show_error(str(error))

    def _control_handler(self, command):
        def handler(_context=None):
            threading.Thread(target=self._run_control, args=(command,), daemon=True).start()
        return handler

    def _control_from_settings(self, command):
        return self._control_handler(command)

    def _seek(self, _view=None):
        try:
            position = float(str(self.get_setting("seek_seconds", "0")).replace(",", "."))
            if position < 0 or position > 86400:
                raise ValueError("Позиция должна быть от 0 до 86400 секунд")
            threading.Thread(target=self._run_control, args=("seek", {"position": position}), daemon=True).start()
        except Exception as error:
            BulletinHelper.show_error(str(error))

    def _set_volume(self, _view=None):
        try:
            volume = float(str(self.get_setting("volume", "80")).replace(",", "."))
            if volume < 0 or volume > 100:
                raise ValueError("Громкость должна быть от 0 до 100")
            threading.Thread(target=self._run_control, args=("set_volume", {"volume": volume / 100.0}), daemon=True).start()
        except Exception as error:
            BulletinHelper.show_error(str(error))

    def _test(self, _view=None):
        try:
            host, port, code = self._values()
        except Exception as error:
            BulletinHelper.show_error(str(error))
            return
        BulletinHelper.show_info("Подключаемся к AERIS…")

        def work():
            try:
                sock = _connect(host, port, code)
                _close_ws(sock)
                BulletinHelper.show_success("AERIS подключён: {}:{}".format(host, port))
            except Exception as error:
                self.logger.exception("AERIS test failed")
                BulletinHelper.show_error(str(error))

        threading.Thread(target=work, daemon=True).start()

    def _on_message_menu(self, context):
        message = context.get("message")
        if message is None:
            BulletinHelper.show_error("Не удалось получить сообщение Telegram")
            return
        try:
            host, port, code = self._values()
            path = _media_path(message, context.get("account"))
        except Exception as error:
            BulletinHelper.show_error(str(error))
            return
        BulletinHelper.show_info("Передаём медиа на компьютер…")

        def work():
            try:
                result = _transfer(host, port, code, path)
                self.logger.info(result)
                BulletinHelper.show_success(result)
            except Exception as error:
                self.logger.exception("AERIS transfer failed")
                BulletinHelper.show_error("Передача не удалась: {}".format(error))

        threading.Thread(target=work, name="AERIS-media-transfer", daemon=True).start()
