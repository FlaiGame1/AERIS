import base64, hashlib, ipaddress, json, mimetypes, os, socket, struct, threading
from base_plugin import BasePlugin, MenuItemData, MenuItemType
from ui.bulletin import BulletinHelper
from ui.settings import Divider, Header, Input, Text

__id__ = "aeris"
__name__ = "AERIS"
__description__ = "Cast Telegram media to AERIS Desktop over your local network."
__author__ = "FlaiGame1"
__version__ = "0.3.2"
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


def _send_msg(sock, kind, payload=None, request_id="aeris"):
    _send_frame(sock, json.dumps({
        "protocol": "aeris",
        "version": 1,
        "type": kind,
        "id": request_id,
        "payload": payload or {},
    }, separators=(",", ":")).encode("utf-8"), 1)
    response = _recv_text(sock)
    if response.get("type") == "error":
        data = response.get("payload", {})
        raise RuntimeError("{}: {}".format(data.get("code"), data.get("message", "")))
    return response


def _close_ws(sock):
    """Finish the WebSocket close handshake, including interleaved control frames."""
    if sock is None:
        return
    try:
        sock.settimeout(2.0)
        _send_frame(sock, struct.pack("!H", 1000), 8)
        # A ping may arrive before the server's close reply. Do not mistake it
        # for the close acknowledgement or tear down TCP prematurely.
        for _ in range(8):
            first, second = _read_exact(sock, 2)
            opcode = first & 15
            size = second & 127
            if size == 126:
                size = struct.unpack("!H", _read_exact(sock, 2))[0]
            elif size == 127:
                size = struct.unpack("!Q", _read_exact(sock, 8))[0]
            if size > 125 and opcode >= 8:
                raise ValueError("Некорректный управляющий WebSocket-кадр")
            mask = _read_exact(sock, 4) if second & 128 else None
            data = _read_exact(sock, size)
            if mask:
                data = bytes(value ^ mask[index % 4] for index, value in enumerate(data))
            if opcode == 8:
                break
            if opcode == 9:
                _send_frame(sock, data, 10)
    except Exception:
        # Closing is best-effort; the transfer/control result is already known.
        pass
    finally:
        try:
            sock.close()
        except Exception:
            pass


def _open_connection(host, port):
    sock = socket.create_connection((host, port), timeout=10)
    sock.settimeout(120)
    key = base64.b64encode(os.urandom(16)).decode("ascii")
    request = (
        "GET /ws HTTP/1.1\r\n"
        "Host: {}:{}\r\n"
        "Upgrade: websocket\r\n"
        "Connection: Upgrade\r\n"
        "Sec-WebSocket-Key: {}\r\n"
        "Sec-WebSocket-Version: 13\r\n\r\n"
    ).format(host, port, key)
    try:
        sock.sendall(request.encode("ascii"))
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
        return sock
    except Exception:
        _close_ws(sock)
        raise


def _connect(host, port, code, device_token=""):
    if device_token:
        sock = None
        try:
            sock = _open_connection(host, port)
            paired = _send_msg(sock, "pair", {"token": device_token}, "pair-token")
            if paired.get("type") == "pair_result" and paired.get("payload", {}).get("paired"):
                return sock, paired.get("payload", {}).get("token", device_token)
        except Exception:
            pass
        _close_ws(sock)

    sock = _open_connection(host, port)
    try:
        paired = _send_msg(sock, "pair", {"code": code}, "pair-code")
        if paired.get("type") != "pair_result" or not paired.get("payload", {}).get("paired"):
            raise ConnectionError("Неверный или истёкший код сопряжения")
        token = paired.get("payload", {}).get("token")
        if not token:
            raise ConnectionError("Компьютер не выдал ключ запоминания устройства")
        return sock, token
    except Exception:
        _close_ws(sock)
        raise


def _media_path(message, account=None):
    from client_utils import get_file_loader
    owner = getattr(message, "messageOwner", None)
    loader = get_file_loader(account) if account is not None else get_file_loader()
    for obj in (message, owner):
        if obj is None:
            continue
        try:
            file_object = loader.getPathToMessage(obj)
            path = str(file_object.getAbsolutePath())
            if os.path.isfile(path):
                return path
        except Exception:
            pass
    raise FileNotFoundError("Файл не найден в кэше Telegram. Сначала полностью скачай видео или аудио.")


def _transfer(host, port, code, path, device_token=""):
    size = os.path.getsize(path)
    if size <= 0 or size > MAX_MEDIA_BYTES:
        raise ValueError("Файл пустой или больше лимита 512 МиБ")
    digest = hashlib.sha256()
    with open(path, "rb") as media:
        while True:
            chunk = media.read(CHUNK_SIZE)
            if not chunk:
                break
            digest.update(chunk)

    sock, device_token = _connect(host, port, code, device_token)
    try:
        mime_type = mimetypes.guess_type(path)[0] or "application/octet-stream"
        response = _send_msg(sock, "begin_media", {
            "file_name": os.path.basename(path),
            "mime_type": mime_type,
            "size": size,
            "sha256": digest.hexdigest(),
        }, "begin")
        if response.get("type") != "media_ready":
            raise RuntimeError("ПК не подготовил приём файла")

        sent = 0
        with open(path, "rb") as media:
            while True:
                chunk = media.read(CHUNK_SIZE)
                if not chunk:
                    break
                _send_frame(sock, chunk, 2)
                sent += len(chunk)

        response = _send_msg(sock, "finish_media", {}, "finish")
        if response.get("type") != "media_complete":
            raise RuntimeError("ПК не подтвердил получение")
        playback = response.get("payload", {}).get("playback")
        if playback != "playing":
            raise RuntimeError("Файл принят, но плеер не запустил воспроизведение. Проверь папку mpv рядом с AERIS Desktop.")
        return "Воспроизведение на ПК: {} ({:.1f} МБ)".format(os.path.basename(path), sent / 1048576), device_token
    finally:
        _close_ws(sock)


class AerisPlugin(BasePlugin):
    def on_plugin_load(self):
        self.logger.info("AERIS 0.3.2 loaded")
        self.media_menu_id = self.add_menu_item(MenuItemData(
            menu_type=MenuItemType.MESSAGE_CONTEXT_MENU,
            text="Воспроизвести через AERIS",
            on_click=self._on_message_menu,
            icon="msg2_devices",
            subtext="Отправить медиа на компьютер",
            priority=100,
        ))

    def on_plugin_unload(self):
        try:
            if getattr(self, "media_menu_id", None):
                self.remove_menu_item(self.media_menu_id)
        except Exception:
            pass

    def create_settings(self):
        return [
            Header(text="AERIS"),
            Text(text="Передача и управление воспроизведением на компьютере", subtext="Телефон и ПК должны быть в одной локальной сети.", icon="msg2_devices"),
            Divider(text="Подключение к компьютеру"),
            Input(key="host", text="IP-адрес компьютера", default=DEFAULT_HOST, subtext="Адрес из окна AERIS Desktop.", icon="msg_text", on_change=self._save_host),
            Input(key="port", text="Порт", default=DEFAULT_PORT, subtext="Стандартный порт — 45872.", icon="msg_settings", on_change=self._save_port),
            Divider(text="Сопряжение устройств"),
            Input(key="pairing_code", text="Код сопряжения", default="", subtext="Нужен только при первом подключении или сбросе сопряжения.", icon="msg_secret", on_change=self._save_code),
            Text(text="Проверить подключение", subtext="Подключиться и запомнить это устройство.", icon="msg_link", accent=True, on_click=self._test),
            Divider(text="Управление плеером"),
            Text(text="Что сейчас играет?", subtext="Получить название, позицию, длительность и состояние плеера.", icon="msg2_devices", on_click=self._get_status),
            Text(text="Продолжить воспроизведение", subtext="Снять паузу на компьютере.", icon="msg2_devices", on_click=self._play),
            Text(text="Поставить на паузу", subtext="Приостановить текущую музыку или видео.", icon="msg2_devices", on_click=self._pause),
            Text(text="Пауза / продолжить", subtext="Переключить состояние воспроизведения.", icon="msg2_devices", on_click=self._toggle_pause),
            Text(text="Остановить воспроизведение", subtext="Остановить текущий файл.", icon="msg2_devices", on_click=self._stop),
            Text(text="Назад на 10 секунд", subtext="Перемотать назад.", icon="msg2_devices", on_click=self._seek_back),
            Text(text="Вперёд на 10 секунд", subtext="Перемотать вперёд.", icon="msg2_devices", on_click=self._seek_forward),
            Text(text="Уменьшить громкость", subtext="−5 пунктов.", icon="msg2_devices", on_click=self._volume_down),
            Text(text="Увеличить громкость", subtext="+5 пунктов.", icon="msg2_devices", on_click=self._volume_up),
            Divider(),
            Text(text="AERIS Plugin 0.3.2", subtext="Медиа передаётся напрямую по локальной сети. Telegram-серверы не участвуют в передаче уже загруженного файла.", icon="msg_info"),
        ]

    def _save_host(self, value):
        self.set_setting("host", str(value).strip())

    def _save_port(self, value):
        self.set_setting("port", str(value).strip())

    def _save_code(self, value):
        self.set_setting("pairing_code", str(value).strip().upper())

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
        if not code and not self.get_setting("device_token", ""):
            raise ValueError("Введи код сопряжения для первого подключения")
        if code and (len(code) != 6 or not code.isalnum()):
            raise ValueError("Код сопряжения должен содержать 6 символов")
        return host, port, code

    def _run_connection_action(self, command=None, payload=None, show_success=True):
        try:
            host, port, code = self._values()
        except Exception as error:
            BulletinHelper.show_error(str(error))
            return
        token = str(self.get_setting("device_token", "")).strip()
        BulletinHelper.show_info("Подключаемся к AERIS…")

        def work():
            sock = None
            try:
                sock, token = _connect(host, port, code, token)
                self.set_setting("device_token", token)
                if command:
                    response = _send_msg(sock, command, payload or {}, "control")
                    if command == "get_status":
                        if response.get("type") != "playback_status":
                            raise RuntimeError("ПК не вернул состояние плеера")
                        state = response.get("payload", {})
                        title = state.get("title") or state.get("path") or "Ничего не воспроизводится"
                        position = state.get("position")
                        duration = state.get("duration")
                        paused = state.get("paused", False)
                        if isinstance(position, (int, float)) and isinstance(duration, (int, float)):
                            def stamp(value):
                                value = max(0, int(value))
                                return "{}:{:02d}".format(value // 60, value % 60)
                            title = "{} · {} / {}".format(title, stamp(position), stamp(duration))
                        title = "{}{}".format("⏸ " if paused else "▶ ", title)
                        BulletinHelper.show_success(str(title)[:180])
                    elif response.get("type") != "command_result" or not response.get("payload", {}).get("success"):
                        raise RuntimeError("ПК не подтвердил выполнение команды")
                if show_success and command != "get_status":
                    BulletinHelper.show_success("AERIS: команда выполнена" if command else "AERIS подключён и запомнил устройство")
            except Exception as error:
                self.logger.exception("AERIS command failed")
                BulletinHelper.show_error(str(error))
            finally:
                _close_ws(sock)

        threading.Thread(target=work, name="AERIS-control", daemon=True).start()

    def _test(self, _view=None):
        self._run_connection_action()

    def _control(self, command, payload=None):
        self._run_connection_action(command, payload)

    def _get_status(self, _view=None):
        self._control("get_status")

    def _play(self, _view=None):
        self._control("play")

    def _pause(self, _view=None):
        self._control("pause")

    def _toggle_pause(self, _view=None):
        self._control("toggle_pause")

    def _stop(self, _view=None):
        self._control("stop")

    def _seek_back(self, _view=None):
        self._control("seek_relative", {"offset": -10})

    def _seek_forward(self, _view=None):
        self._control("seek_relative", {"offset": 10})

    def _volume_down(self, _view=None):
        self._control("volume_delta", {"delta": -5})

    def _volume_up(self, _view=None):
        self._control("volume_delta", {"delta": 5})

    def _on_message_menu(self, context):
        self.logger.info("AERIS menu context keys: {}".format(list(context.keys())))
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

        token = str(self.get_setting("device_token", "")).strip()
        BulletinHelper.show_info("Передаём медиа и запускаем воспроизведение на ПК…")

        def work():
            try:
                result, token = _transfer(host, port, code, path, token)
                self.set_setting("device_token", token)
                self.logger.info(result)
                BulletinHelper.show_success(result)
            except Exception as error:
                self.logger.exception("AERIS transfer failed")
                BulletinHelper.show_error("Передача не удалась: {}".format(error))

        threading.Thread(target=work, name="AERIS-media-transfer", daemon=True).start()
