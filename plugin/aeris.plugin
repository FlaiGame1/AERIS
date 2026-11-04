import base64, hashlib, ipaddress, json, os, socket, struct, threading
from base_plugin import BasePlugin, MenuItemData, MenuItemType
from ui.bulletin import BulletinHelper
from ui.settings import Divider, Header, Input, Text

__id__="aeris"
__name__="AERIS"
__description__="Cast Telegram media to AERIS Desktop over your local network."
__author__="FlaiGame1"
__version__="0.2.0"
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
def _connect(host,port,code):
 s=socket.create_connection((host,port),timeout=10);s.settimeout(30)
 key=base64.b64encode(os.urandom(16)).decode()
 req="GET /ws HTTP/1.1\r\nHost: {}:{}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {}\r\nSec-WebSocket-Version: 13\r\n\r\n".format(host,port,key)
 s.sendall(req.encode());resp=b""
 while b"\r\n\r\n" not in resp:
  x=s.recv(1024)
  if not x:s.close();raise ConnectionError("Нет ответа WebSocket")
  resp+=x
  if len(resp)>8192:s.close();raise ValueError("Некорректный ответ WebSocket")
 if b" 101 " not in resp.split(b"\r\n",1)[0]:s.close();raise ConnectionError("AERIS отклонил WebSocket")
 if _send_msg(s,"hello").get("type")!="hello_ack":s.close();raise ConnectionError("Неожиданный ответ AERIS")
 p=_send_msg(s,"pair",{"code":code})
 if p.get("type")!="pair_result" or not p.get("payload",{}).get("paired"):s.close();raise ConnectionError("Неверный или истёкший код сопряжения")
 return s
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
 raise FileNotFoundError("Файл не найден в кэше Telegram. Сначала полностью скачай видео или аудио.")
def _transfer(host,port,code,path):
 size=os.path.getsize(path)
 if size<=0 or size>MAX_MEDIA_BYTES:raise ValueError("Файл пустой или больше лимита 512 МиБ")
 h=hashlib.sha256()
 with open(path,"rb") as f:
  while True:
   b=f.read(CHUNK_SIZE)
   if not b:break
   h.update(b)
 s=_connect(host,port,code)
 try:
  r=_send_msg(s,"begin_media",{"file_name":os.path.basename(path),"mime_type":"application/octet-stream","size":size,"sha256":h.hexdigest()},"begin")
  if r.get("type")!="media_ready":raise RuntimeError("ПК не подготовил приём файла")
  sent=0
  with open(path,"rb") as f:
   while True:
    b=f.read(CHUNK_SIZE)
    if not b:break
    _send_frame(s,b,2);sent+=len(b)
  r=_send_msg(s,"finish_media",{},"finish")
  if r.get("type")!="media_complete":raise RuntimeError("ПК не подтвердил получение")
  return "Передано на ПК: {} ({:.1f} МБ)".format(os.path.basename(path),sent/1048576)
 finally:s.close()

class AerisPlugin(BasePlugin):
 def on_plugin_load(self):
  self.logger.info("AERIS 0.2.0 loaded")
  self.media_menu_id=self.add_menu_item(MenuItemData(menu_type=MenuItemType.MESSAGE_CONTEXT_MENU,text="Передать через AERIS",on_click=self._on_message_menu,icon="msg2_devices",subtext="Воспроизвести на компьютере",priority=100))
 def on_plugin_unload(self):
  try:
   if getattr(self,"media_menu_id",None):self.remove_menu_item(self.media_menu_id)
  except Exception:pass
 def create_settings(self):
  return [Header(text="AERIS"),Text(text="Передача видео и аудио из Telegram на компьютер",subtext="Телефон и ПК должны быть в одной локальной сети.",icon="msg2_devices"),Divider(text="Подключение к компьютеру"),Input(key="host",text="IP-адрес компьютера",default=DEFAULT_HOST,subtext="Адрес из окна AERIS Desktop.",icon="msg_text",on_change=self._save_host),Input(key="port",text="Порт",default=DEFAULT_PORT,subtext="Стандартный порт — 45872.",icon="msg_settings",on_change=self._save_port),Divider(text="Сопряжение устройств"),Input(key="pairing_code",text="Код сопряжения",default="",subtext="Введите 6-значный код из AERIS Desktop.",icon="msg_secret",on_change=self._save_code),Text(text="Проверить подключение",subtext="Проверка связи и сопряжения.",icon="msg_link",accent=True,on_click=self._test),Divider(),Text(text="AERIS 0.2.0 · Передача файлов",subtext="Тестовая версия. Используй только доверенную домашнюю сеть.",icon="msg_info")]
 def _save_host(self,v):self.set_setting("host",str(v).strip())
 def _save_port(self,v):self.set_setting("port",str(v).strip())
 def _save_code(self,v):self.set_setting("pairing_code",str(v).strip().upper())
 def _values(self):
  host=str(self.get_setting("host",DEFAULT_HOST)).strip();port=int(str(self.get_setting("port",DEFAULT_PORT)).strip());code=str(self.get_setting("pairing_code","")).strip().upper()
  if not host or any(c.isspace() for c in host):raise ValueError("Укажи IP-адрес компьютера")
  try:ipaddress.ip_address(host)
  except ValueError:
   if len(host)>253 or any(not p or len(p)>63 for p in host.split(".")):raise ValueError("Проверь IP-адрес")
  if not 1<=port<=65535:raise ValueError("Неверный порт")
  if len(code)!=6 or not code.isalnum():raise ValueError("Введи 6-значный код сопряжения")
  return host,port,code
 def _test(self,_view=None):
  try:host,port,code=self._values()
  except Exception as e:BulletinHelper.show_error(str(e));return
  BulletinHelper.show_info("Подключаемся к AERIS…")
  def work():
   try:s=_connect(host,port,code);s.close();BulletinHelper.show_success("AERIS подключён: {}:{}".format(host,port))
   except Exception as e:self.logger.exception("AERIS test failed");BulletinHelper.show_error(str(e))
  threading.Thread(target=work,daemon=True).start()
 def _on_message_menu(self,context):
  self.logger.info("AERIS menu context keys: {}".format(list(context.keys())))
  msg=context.get("message")
  if msg is None:BulletinHelper.show_error("Не удалось получить сообщение Telegram");return
  try:host,port,code=self._values();path=_media_path(msg,context.get("account"))
  except Exception as e:BulletinHelper.show_error(str(e));return
  BulletinHelper.show_info("Передаём медиа на компьютер…")
  def work():
   try:r=_transfer(host,port,code,path);self.logger.info(r);BulletinHelper.show_success(r)
   except Exception as e:self.logger.exception("AERIS transfer failed");BulletinHelper.show_error("Передача не удалась: {}".format(e))
  threading.Thread(target=work,name="AERIS-media-transfer",daemon=True).start()
