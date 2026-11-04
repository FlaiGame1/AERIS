import base64, hashlib, ipaddress, json, mimetypes, os, socket, struct, threading
from base_plugin import BasePlugin, MenuItemData, MenuItemType
from ui.bulletin import BulletinHelper
from ui.settings import Divider, Header, Input, Text

__id__="aeris"
__name__="AERIS"
__description__="Cast Telegram media to AERIS Desktop over your local network."
__author__="FlaiGame1"
__version__="0.3.0"
__icon__="exteraPlugins/1"
__app_version__=">=12.5.1"
__sdk_version__=">=1.4.4.3"
__category__=["media","integrations"]
DEFAULT_HOST="192.168.1.100"
DEFAULT_PORT="45872"
CHUNK_SIZE=1024*1024
MAX_MEDIA_BYTES=512*1024*1024

def _read_exact(sock,n):
 data=bytearray()
 while len(data)<n:
  part=sock.recv(n-len(data))
  if not part: raise ConnectionError("Компьютер закрыл соединение")
  data.extend(part)
 return bytes(data)
def _send_frame(sock,payload,opcode=1):
 mask=os.urandom(4);n=len(payload);head=bytearray([0x80|opcode])
 if n<126: head.append(0x80|n)
 elif n<65536: head.append(0x80|126);head.extend(struct.pack("!H",n))
 else: head.append(0x80|127);head.extend(struct.pack("!Q",n))
 masked=bytes(v^mask[i%4] for i,v in enumerate(payload));sock.sendall(bytes(head)+mask+masked)
def _recv_text(sock):
 while True:
  a,b=_read_exact(sock,2);op=a&15;n=b&127
  if n==126:n=struct.unpack("!H",_read_exact(sock,2))[0]
  elif n==127:n=struct.unpack("!Q",_read_exact(sock,8))[0]
  if n>65536:raise ValueError("Ответ AERIS слишком большой")
  mask=_read_exact(sock,4) if b&128 else None;data=_read_exact(sock,n)
  if mask:data=bytes(v^mask[i%4] for i,v in enumerate(data))
  if op==8:raise ConnectionError("Компьютер закрыл WebSocket")
  if op==9:_send_frame(sock,data,10);continue
  if op==1:return json.loads(data.decode("utf-8"))
def _send_msg(sock,kind,payload=None,req="aeris"):
 _send_frame(sock,json.dumps({"protocol":"aeris","version":1,"type":kind,"id":req,"payload":payload or {}},separators=(",",":")).encode(),1)
 r=_recv_text(sock)
 if r.get("type")=="error":
  p=r.get("payload",{});raise RuntimeError("{}: {}".format(p.get("code"),p.get("message","")))
 return r
def _graceful_close(sock):
 if sock is None:return
 try:
  _send_frame(sock,b"",8);sock.settimeout(1)
  a,b=_read_exact(sock,2);n=b&127
  if n==126:n=struct.unpack("!H",_read_exact(sock,2))[0]
  elif n==127:n=struct.unpack("!Q",_read_exact(sock,8))[0]
  if b&128:_read_exact(sock,4)
  if n:_read_exact(sock,n)
 except Exception:pass
 try:sock.close()
 except Exception:pass
def _connect(host,port,code):
 s=socket.create_connection((host,port),timeout=10);s.settimeout(60)
 try:
  key=base64.b64encode(os.urandom(16)).decode()
  req="GET /ws HTTP/1.1\r\nHost: {}:{}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {}\r\nSec-WebSocket-Version: 13\r\n\r\n".format(host,port,key)
  s.sendall(req.encode());resp=b""
  while b"\r\n\r\n" not in resp:
   x=s.recv(1024)
   if not x:raise ConnectionError("Нет ответа WebSocket")
   resp+=x
   if len(resp)>8192:raise ValueError("Некорректный ответ WebSocket")
  if b" 101 " not in resp.split(b"\r\n",1)[0]:raise ConnectionError("AERIS отклонил WebSocket")
  if _send_msg(s,"hello").get("type")!="hello_ack":raise ConnectionError("Неожиданный ответ AERIS")
  p=_send_msg(s,"pair",{"code":code})
  if p.get("type")!="pair_result" or not p.get("payload",{}).get("paired"):raise ConnectionError("Неверный или истёкший код сопряжения")
  return s
 except Exception:
  _graceful_close(s);raise
def _media_path(message,account=None):
 from client_utils import get_file_loader
 owner=getattr(message,"messageOwner",None)
 loader=get_file_loader(account) if account is not None else get_file_loader()
 for obj in (message,owner):
  if obj is None:continue
  try:
   f=loader.getPathToMessage(obj);p=str(f.getAbsolutePath())
   if os.path.isfile(p):return p
  except Exception:pass
 raise FileNotFoundError("Файл не найден в кэше Telegram. Сначала полностью скачай медиа в Telegram.")
def _transfer(sock,path):
 size=os.path.getsize(path)
 if size<=0 or size>MAX_MEDIA_BYTES:raise ValueError("Файл пустой или больше лимита 512 МиБ")
 h=hashlib.sha256()
 with open(path,"rb") as f:
  while True:
   b=f.read(CHUNK_SIZE)
   if not b:break
   h.update(b)
 mime=mimetypes.guess_type(path)[0] or "application/octet-stream"
 r=_send_msg(sock,"begin_media",{"file_name":os.path.basename(path),"mime_type":mime,"size":size,"sha256":h.hexdigest()},"begin")
 if r.get("type")!="media_ready":raise RuntimeError("ПК не подготовил приём файла")
 sent=0
 with open(path,"rb") as f:
  while True:
   b=f.read(CHUNK_SIZE)
   if not b:break
   _send_frame(sock,b,2);sent+=len(b)
 r=_send_msg(sock,"finish_media",{},"finish")
 if r.get("type")!="media_complete":raise RuntimeError("ПК не подтвердил получение")
 return "Передано на ПК: {} ({:.1f} МБ)".format(os.path.basename(path),sent/1048576)

class AerisPlugin(BasePlugin):
 def on_plugin_load(self):
  self.logger.info("AERIS 0.3.0 loaded")
  self._socket=None
  self._socket_lock=threading.RLock()
  self.media_menu_id=self.add_menu_item(MenuItemData(menu_type=MenuItemType.MESSAGE_CONTEXT_MENU,text="Воспроизвести через AERIS",on_click=self._on_message_menu,icon="msg2_devices",subtext="Передать медиа на компьютер",priority=100))
 def on_plugin_unload(self):
  try:
   if getattr(self,"media_menu_id",None):self.remove_menu_item(self.media_menu_id)
  except Exception:pass
  with getattr(self,"_socket_lock",threading.RLock()):
   _graceful_close(getattr(self,"_socket",None));self._socket=None
 def create_settings(self):
  return [Header(text="AERIS"),Text(text="Передача и управление медиа на компьютере",subtext="Телефон и ПК должны быть в одной локальной сети.",icon="msg2_devices"),Divider(text="Подключение к компьютеру"),Input(key="host",text="IP-адрес компьютера",default=DEFAULT_HOST,subtext="Адрес из окна AERIS Desktop.",icon="msg_text",on_change=self._save_host),Input(key="port",text="Порт",default=DEFAULT_PORT,subtext="Стандартный порт — 45872.",icon="msg_settings",on_change=self._save_port),Divider(text="Сопряжение устройств"),Input(key="pairing_code",text="Код сопряжения",default="",subtext="Введите код из AERIS Desktop. Он действует до остановки приёмника; после обрыва соединение переподключится автоматически.",icon="msg_secret",on_change=self._save_code),Text(text="Проверить подключение",subtext="Подключение останется открытым для команд.",icon="msg_link",accent=True,on_click=self._test),Divider(text="Управление плеером"),Text(text="▶ Воспроизвести / продолжить",on_click=self._play),Text(text="⏸ Пауза",on_click=self._pause),Text(text="⏹ Остановить",on_click=self._stop),Input(key="volume",text="Громкость (0–100)",default="70",on_change=self._save_volume),Text(text="Применить громкость",on_click=self._set_volume),Input(key="seek_seconds",text="Перемотка (секунды)",default="0",on_change=self._save_seek),Text(text="Перемотать",on_click=self._seek),Text(text="Получить состояние плеера",on_click=self._status),Divider(),Text(text="AERIS 0.3.0 · Нативное аудио и удалённое управление",subtext="Пока поддерживается встроенное аудио; видео открывается системным проигрывателем.",icon="msg_info")]
 def _save_host(self,v):self.set_setting("host",str(v).strip())
 def _save_port(self,v):self.set_setting("port",str(v).strip())
 def _save_code(self,v):self.set_setting("pairing_code",str(v).strip().upper())
 def _save_volume(self,v):self.set_setting("volume",str(v).strip())
 def _save_seek(self,v):self.set_setting("seek_seconds",str(v).strip())
 def _values(self):
  host=str(self.get_setting("host",DEFAULT_HOST)).strip();port=int(str(self.get_setting("port",DEFAULT_PORT)).strip());code=str(self.get_setting("pairing_code","")).strip().upper()
  if not host or any(c.isspace() for c in host):raise ValueError("Укажи IP-адрес компьютера")
  try:ipaddress.ip_address(host)
  except ValueError:
   if len(host)>253 or any(not p or len(p)>63 for p in host.split(".")):raise ValueError("Проверь IP-адрес")
  if not 1<=port<=65535:raise ValueError("Неверный порт")
  if len(code)!=6 or not code.isalnum():raise ValueError("Введи 6-значный код сопряжения")
  return host,port,code
 def _ensure_socket(self):
  if self._socket is not None:return self._socket
  host,port,code=self._values()
  self._socket=_connect(host,port,code)
  return self._socket
 def _command(self,kind,payload=None):
  last=None
  with self._socket_lock:
   for attempt in range(2):
    try:
     sock=self._ensure_socket()
     return _send_msg(sock,kind,payload or {},kind)
    except Exception as e:
     last=e;_graceful_close(self._socket);self._socket=None
     if attempt:break
  raise last
 def _run_control(self,kind,payload=None):
  BulletinHelper.show_info("Отправляем команду AERIS…")
  def work():
   try:
    r=self._command(kind,payload)
    p=r.get("payload",{})
    if r.get("type")=="player_status" or "player" in p:
     state=p.get("player",p);msg="{} · {} · {:.1f} сек".format(state.get("title") or "Без трека",state.get("state","неизвестно"),float(state.get("position_seconds",0)))
    else:msg="Команда AERIS выполнена"
    BulletinHelper.show_success(msg)
   except Exception as e:
    self.logger.exception("AERIS command failed")
    BulletinHelper.show_error("AERIS: {}".format(e))
  threading.Thread(target=work,name="AERIS-player-control",daemon=True).start()
 def _play(self,_view=None):self._run_control("play")
 def _pause(self,_view=None):self._run_control("pause")
 def _stop(self,_view=None):self._run_control("stop")
 def _set_volume(self,_view=None):
  try:v=float(str(self.get_setting("volume","70")).replace(",","."))/100.0
  except Exception:BulletinHelper.show_error("Введи громкость от 0 до 100");return
  if not 0<=v<=1:BulletinHelper.show_error("Громкость должна быть от 0 до 100");return
  self._run_control("set_volume",{"volume":v})
 def _seek(self,_view=None):
  try:v=float(str(self.get_setting("seek_seconds","0")).replace(",","."))
  except Exception:BulletinHelper.show_error("Введи позицию в секундах");return
  if v<0:BulletinHelper.show_error("Позиция не может быть отрицательной");return
  self._run_control("seek",{"position":v})
 def _status(self,_view=None):self._run_control("get_status")
 def _test(self,_view=None):
  try:self._values()
  except Exception as e:BulletinHelper.show_error(str(e));return
  def work():
   try:
    r=self._command("ping")
    if r.get("type")!="pong":raise RuntimeError("Неожиданный ответ AERIS")
    BulletinHelper.show_success("AERIS подключён. Соединение оставлено активным.")
   except Exception as e:BulletinHelper.show_error(str(e))
  threading.Thread(target=work,daemon=True).start()
 def _on_message_menu(self,context):
  self.logger.info("AERIS menu context keys: {}".format(list(context.keys())))
  msg=context.get("message")
  if msg is None:BulletinHelper.show_error("Не удалось получить сообщение Telegram");return
  try:_,_,_=self._values();path=_media_path(msg,context.get("account"))
  except Exception as e:BulletinHelper.show_error(str(e));return
  BulletinHelper.show_info("Передаём медиа на компьютер…")
  def work():
   try:
    with self._socket_lock:
     sock=self._ensure_socket()
     result=_transfer(sock,path)
    self.logger.info(result);BulletinHelper.show_success(result)
   except Exception as e:
    self.logger.exception("AERIS transfer failed")
    with self._socket_lock:
     _graceful_close(self._socket);self._socket=None
    BulletinHelper.show_error("Передача не удалась: {}".format(e))
  threading.Thread(target=work,name="AERIS-media-transfer",daemon=True).start()
