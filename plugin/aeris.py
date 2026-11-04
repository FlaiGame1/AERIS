import base64, hashlib, ipaddress, json, mimetypes, os, socket, struct, threading
from base_plugin import BasePlugin, MenuItemData, MenuItemType, MethodHook
from ui.bulletin import BulletinHelper
from ui.settings import Divider, Header, Input, Text

__id__ = "aeris"
__name__ = "AERIS"
__description__ = "Cast Telegram media to AERIS Desktop over your local network."
__author__ = "FlaiGame1"
__version__ = "1.0.0"
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


def _file_object_path(file_object):
    """Return a readable plaintext path from Telegram's own FileLoader result."""
    if file_object is None:
        return None

    path = None
    try:
        if hasattr(file_object, "getAbsolutePath"):
            path = str(file_object.getAbsolutePath())
        elif hasattr(file_object, "getPath"):
            path = str(file_object.getPath())
        else:
            path = os.fspath(str(file_object))
    except Exception:
        return None

    if not path:
        return None

    # A client may keep an encrypted cache blob next to its plaintext target.
    # Never send the raw encrypted blob to the desktop player as if it were media.
    if path.lower().endswith(".enc"):
        raise ValueError("Telegram FileLoader вернул зашифрованный .enc-файл, а не готовое медиа")

    try:
        exists = file_object.exists() if hasattr(file_object, "exists") else os.path.isfile(path)
        is_file = file_object.isFile() if hasattr(file_object, "isFile") else os.path.isfile(path)
        size = int(file_object.length()) if hasattr(file_object, "length") else os.path.getsize(path)
        readable = os.access(path, os.R_OK)
        if exists and is_file and size > 0 and readable:
            return os.path.abspath(path)
    except Exception:
        pass
    return None


def _media_path(message_or_document, account=None):
    """Resolve media through the active Telegram client's FileLoader.

    No AyuGram/exteraGram-specific storage directory is assumed. We ask the
    client for the message path first, then for its underlying attachment.
    Only already-local files are accepted; this function never starts a download.
    """
    from client_utils import get_file_loader

    def safe_get(obj, name):
        try:
            return getattr(obj, name, None)
        except Exception:
            return None

    owner = safe_get(message_or_document, "messageOwner")
    message_candidates = []
    attachment_candidates = []

    for candidate in (message_or_document, owner):
        if candidate is not None and candidate not in message_candidates:
            message_candidates.append(candidate)

        media = safe_get(candidate, "media") if candidate is not None else None
        document = safe_get(media, "document") if media is not None else None
        if document is not None and document not in attachment_candidates:
            attachment_candidates.append(document)

        webpage = safe_get(media, "webpage") if media is not None else None
        web_document = safe_get(webpage, "document") if webpage is not None else None
        if web_document is not None and web_document not in attachment_candidates:
            attachment_candidates.append(web_document)

        # Some plugin callbacks pass a document directly instead of a MessageObject.
        if candidate is not None and safe_get(candidate, "id") is not None:
            class_name = str(type(candidate))
            if "Document" in class_name and candidate not in attachment_candidates:
                attachment_candidates.append(candidate)

        getter = safe_get(candidate, "getDocument") if candidate is not None else None
        if callable(getter):
            try:
                document = getter()
                if document is not None and document not in attachment_candidates:
                    attachment_candidates.append(document)
            except Exception:
                pass

    try:
        loader = get_file_loader(account) if account is not None else get_file_loader()
    except Exception:
        loader = get_file_loader()

    attempted = []
    for message in message_candidates:
        try:
            file_object = loader.getPathToMessage(message)
            path = _file_object_path(file_object)
            if path:
                return path
            attempted.append("getPathToMessage: путь отсутствует или файл недоступен")
        except Exception as error:
            attempted.append("getPathToMessage: {}".format(type(error).__name__))

    for attachment in attachment_candidates:
        try:
            file_object = loader.getPathToAttach(attachment)
            path = _file_object_path(file_object)
            if path:
                return path
            attempted.append("getPathToAttach: путь отсутствует или файл недоступен")
        except Exception as error:
            attempted.append("getPathToAttach: {}".format(type(error).__name__))

        # Telegram's FileLoader also exposes a force-cache overload. It is a
        # lookup only; it does not initiate a network download.
        try:
            file_object = loader.getPathToAttach(attachment, True)
            path = _file_object_path(file_object)
            if path:
                return path
        except Exception as error:
            attempted.append("getPathToAttach(forceCache): {}".format(type(error).__name__))

    detail = "; ".join(attempted[-6:]) or "в объекте сообщения не найден документ"
    raise FileNotFoundError(
        "FileLoader клиента не вернул доступный локальный файл. "
        "AERIS не ищет папки AyuGram/exteraGram и не читает зашифрованные blobs. "
        "Диагностика: {}".format(detail)
    )
def _media_metadata(message):
    """Read visible Telegram audio tags from the selected message/document."""
    result = {"track_title": "", "track_artist": ""}
    try:
        owner = getattr(message, "messageOwner", None)
        candidates = [message, owner]
        for candidate in list(candidates):
            media = getattr(candidate, "media", None) if candidate is not None else None
            document = getattr(media, "document", None) if media is not None else None
            if document is not None:
                candidates.append(document)
        for candidate in candidates:
            attributes = getattr(candidate, "attributes", None)
            if attributes is None:
                continue
            for attribute in attributes:
                if "AudioAttribute" not in str(type(attribute)) and not hasattr(attribute, "performer"):
                    continue
                title = str(getattr(attribute, "title", "") or "").strip()
                artist = str(getattr(attribute, "performer", "") or "").strip()
                if title:
                    result["track_title"] = title
                if artist:
                    result["track_artist"] = artist
                if title or artist:
                    return result
    except Exception:
        pass
    return result


def _transfer(host, port, code, path, device_token="", metadata=None):
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
            "track_title": (metadata or {}).get("track_title", ""),
            "track_artist": (metadata or {}).get("track_artist", ""),
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
        self.logger.info("AERIS 1.0.0 loaded")
        self.media_menu_id = self.add_menu_item(MenuItemData(
            menu_type=MenuItemType.MESSAGE_CONTEXT_MENU,
            text="Воспроизвести через AERIS",
            on_click=self._on_message_menu,
            icon="msg2_devices",
            subtext="Отправить медиа на компьютер",
            priority=100,
        ))
        self.profile_menu_id = self.add_menu_item(MenuItemData(
            menu_type=MenuItemType.PROFILE_ACTION_MENU,
            text="Передать музыку профиля через AERIS",
            on_click=self._on_profile_menu,
            icon="msg2_devices",
            subtext="Отправить сохранённый профильный трек на ПК",
            priority=100,
        ))
        self._player_menu_action = None
        self._injected_player_menus = set()
        self._install_player_menu_hook()

    def on_plugin_unload(self):
        try:
            if getattr(self, "media_menu_id", None):
                self.remove_menu_item(self.media_menu_id)
            if getattr(self, "profile_menu_id", None):
                self.remove_menu_item(self.profile_menu_id)
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
            Text(text="AERIS Plugin 0.3.9", subtext="Медиа передаётся напрямую по локальной сети. Telegram-серверы не участвуют в передаче уже загруженного файла.", icon="msg_info"),
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
                sock, new_token = _connect(host, port, code, token)
                self.set_setting("device_token", new_token)
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


    def _send_path(self, path, label="медиа", metadata=None):
        try:
            host, port, code = self._values()
        except Exception as error:
            BulletinHelper.show_error(str(error))
            return
        token = str(self.get_setting("device_token", "")).strip()
        BulletinHelper.show_info("Передаём {} на AERIS Desktop…".format(label))

        def work():
            try:
                result, new_token = _transfer(host, port, code, path, token, metadata)
                self.set_setting("device_token", new_token)
                self.logger.info(result)
                BulletinHelper.show_success(result)
            except Exception as error:
                self.logger.exception("AERIS media transfer failed")
                BulletinHelper.show_error("Не удалось передать {}: {}".format(label, error))

        threading.Thread(target=work, name="AERIS-media-transfer", daemon=True).start()


    def _install_player_menu_hook(self):
        try:
            from hook_utils import find_class
            item_options = find_class("org.telegram.ui.Components.ItemOptions")
            if not item_options:
                self.logger.info("AERIS: ItemOptions class unavailable; skipping native player menu")
                return
            show_method = item_options.getDeclaredMethod("show")
            show_method.setAccessible(True)
            self._player_menu_unhook = self.hook_method(
                show_method, _AerisItemOptionsHook(self), priority=100
            )
            self.logger.info("AERIS: native player menu hook registered")
        except Exception:
            self.logger.exception("AERIS: native player menu hook unavailable")

    def _inject_player_menu_item(self, options):
        try:
            from java.lang import Thread as JavaThread
            frames = JavaThread.currentThread().getStackTrace()
            in_player_menu = any(
                str(frame.getClassName()).endswith(".AudioPlayerAlert")
                and str(frame.getMethodName()) == "showMenuOptions"
                for frame in frames
            )
            if not in_player_menu:
                return
            key = int(options.hashCode())
            if key in self._injected_player_menus:
                return
            if self._player_menu_action is None:
                from extera_utils.classes import Base, java_subclass, joverride
                from java.lang import Object, Runnable
                plugin = self

                @java_subclass(Object, Runnable)
                class AerisPlayerMenuAction(Base):
                    def __init__(self, plugin_instance):
                        self.plugin = plugin_instance

                    @joverride()
                    def run(self):
                        self.plugin._send_current_player_media()

                self._player_menu_action = AerisPlayerMenuAction.new_instance(init_args=[plugin])
            options.add(0, "Воспроизвести через AERIS", self._player_menu_action.java)
            self._injected_player_menus.add(key)
            self.logger.info("AERIS: injected action into Telegram audio player menu")
        except Exception:
            self.logger.exception("AERIS: failed to add player menu action")

    def _send_current_player_media(self):
        try:
            from hook_utils import find_class
            media_controller = find_class("org.telegram.messenger.MediaController")
            if not media_controller:
                raise RuntimeError("MediaController Telegram не найден")
            message = media_controller.getInstance().getPlayingMessageObject()
            if message is None:
                raise RuntimeError("Сейчас в Telegram ничего не воспроизводится")
            account = getattr(message, "currentAccount", None)
            path = _media_path(message, account)
            self._send_path(path, "текущий трек", _media_metadata(message))
        except Exception as error:
            self.logger.exception("AERIS current player transfer failed")
            BulletinHelper.show_error("Не удалось передать текущий трек: {}".format(error))

    def _on_profile_menu(self, context):
        self.logger.info("AERIS profile menu context keys: {}".format(list(context.keys())))
        account = context.get("account")
        candidates = []
        for owner in (context.get("userFull"), context.get("user")):
            if owner is None:
                continue
            candidates.append(owner)
            for field in ("saved_music", "savedMusic", "music", "profileMusic", "profile_music", "audio", "media", "message"):
                try:
                    value = getattr(owner, field, None)
                    if value is not None:
                        candidates.append(value)
                except Exception:
                    pass
        for candidate in candidates:
            try:
                path = _media_path(candidate, account)
                self._send_path(path, "музыку профиля")
                return
            except Exception:
                continue
        BulletinHelper.show_error("Не удалось найти загруженный файл профильной музыки. Сначала открой трек в Telegram и дождись его загрузки.")

    def _on_message_menu(self, context):
        # This notification is deliberately the first operation: if it does
        # not appear, the client never invoked the registered menu callback.
        BulletinHelper.show_info("AERIS: нажата команда передачи")
        try:
            try:
                keys = list(context.keys()) if context is not None and hasattr(context, "keys") else []
            except Exception:
                keys = []
            self.logger.info("AERIS message action invoked; context type={}, keys={}".format(type(context).__name__, keys))

            message = None
            account = None
            if context is not None:
                getter = getattr(context, "get", None)
                if callable(getter):
                    message = getter("message")
                    account = getter("account")
                    if message is None:
                        grouped = getter("groupedMessages")
                        if grouped:
                            try:
                                message = grouped[0]
                            except Exception:
                                message = None

            if message is None:
                raise RuntimeError(
                    "Обработчик AERIS вызван, но Telegram не передал context['message']. "
                    "Ключи контекста: {}".format(", ".join(keys) or "нет")
                )

            self.logger.info("AERIS selected message object: {}".format(type(message).__name__))
            host, port, code = self._values()
            path = _media_path(message, account)
            self.logger.info("AERIS resolved local media path: {}".format(path))
            self._send_path(path, "медиа", _media_metadata(message))
        except Exception as error:
            self.logger.exception("AERIS message menu action failed")
            BulletinHelper.show_error("AERIS: {}".format(error))


class _AerisItemOptionsHook(MethodHook):
    def __init__(self, plugin):
        self.plugin = plugin

    def before_hooked_method(self, param):
        self.plugin._inject_player_menu_item(param.thisObject)
