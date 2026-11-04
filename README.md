# AERIS

**Current version: 1.0.0** · First public release

**Open-source media casting ecosystem / Открытая экосистема трансляции медиа**

[English](#english) · [Русский](#русский)

---

## English

AERIS is an open-source project for sending media from Android to a Windows PC over a local network — without relying on Google Chromecast or Telegram servers for the actual transfer.

### Components

- **AERIS Desktop** — a Windows receiver and playback controller, written in Rust.
- **AERIS Plugin** — a Python plugin for AyuGram / exteraGram that sends cached Telegram media and provides remote playback controls.
- **AERIS Protocol** — a shared WebSocket protocol for pairing, media transfer and playback commands.

### Features in 1.0.0

- Direct phone-to-PC transfer over the local Wi-Fi network.
- First-time pairing with a short code and remembered device tokens.
- Chunked transfer (1 MiB chunks) with SHA-256 integrity verification.
- Audio and video playback through the bundled mpv engine.
- Phone controls: pause/resume, stop, seek backward/forward and volume up/down.
- Telegram plugin action in the long-press message context menu; additional audio-player and profile menu integrations are best-effort and depend on the Telegram client build.
- Remote playback status: current title, playback position, duration and pause state.
- Received files are stored under the Windows temporary AERIS folder.
- Standalone browser uploader on the PC (port 45873) for sending local files from Android without Telegram.

### Development status

AERIS 1.0.0 is the first public release. It provides direct Android-to-Windows media transfer, playback and remote controls over a local network. The Android side is a Telegram plugin only; there is no separate Android application. The plugin asks the active Telegram client's FileLoader for an already downloaded media file, so media must be available locally before sending. The transfer itself is direct over the local network and does not pass through Telegram servers. If Telegram is blocked but the file is already cached and both devices can communicate over the LAN, the transfer can still work. When Telegram cannot provide the file, the browser uploader can send a file selected from Android storage.

The long-press message action is the verified Telegram transfer entry point. The plugin also contains best-effort hooks for the native audio-player overflow menu and profile music; these integrations are client-version-sensitive and are not guaranteed on every AyuGram/exteraGram build. The video message's own three-dot menu is not yet implemented. AERIS Desktop serves a browser upload page at http://<PC-LAN-IP>:45873 for direct file selection on Android. Windows playback currently uses a separate mpv window. Transfer resume, transport encryption, device-management UI, and a native video-message overflow action are not part of this candidate yet.

### Security

The current protocol uses unencrypted WebSocket (ws://) on the private LAN. The receiver rejects peers whose source IP is not private, loopback or link-local, and pairing tokens are stored locally by the plugin and Desktop. This is defense in depth, not encryption: use only a trusted home network and never expose TCP ports 45872 or 45873 to the public internet.

### Repository structure

AERIS/
- desktop/ — Rust Windows receiver and player controller
- plugin/ — Python AyuGram / exteraGram plugin
- protocol/ — shared protocol specification
- licenses/ — third-party license notices
- .github/workflows/ — CI and release builds

### Releases

The 1.0.0 Windows package is published through GitHub Releases. The Android component is distributed as plugin source/package for compatible clients:
- **Windows:** portable ZIP with AERIS Desktop and the bundled mpv runtime.
- **Android:** AERIS Telegram plugin source/package for compatible AyuGram / exteraGram clients.

### Contributing

Issues, bug reports and contributions are welcome. Include your Windows version, Android client version and relevant AERIS logs when reporting problems.

---

## Русский

AERIS — открытый проект для передачи видео и музыки с Android на компьютер с Windows по локальной сети. Для самой передачи не нужны серверы Google Chromecast или Telegram.

### Компоненты

- **AERIS Desktop** — приёмник и контроллер воспроизведения для Windows на Rust.
- **AERIS Plugin** — Python-плагин для AyuGram / exteraGram, который отправляет медиа из кэша Telegram и даёт управлять воспроизведением с телефона.
- **AERIS Protocol** — общий WebSocket-протокол для сопряжения устройств, передачи медиа и команд плеера.

### Возможности версии 1.0.0

- Прямая передача с телефона на ПК через локальную Wi-Fi-сеть.
- Первичное сопряжение по короткому коду и запоминание устройства по токену.
- Передача блоками по 1 МиБ с проверкой целостности SHA-256.
- Воспроизведение аудио и видео через движок mpv, включённый в поставку.
- Управление с телефона: пауза/продолжение, остановка, перемотка назад/вперёд и изменение громкости.
- Подтверждённое действие плагина в контекстном меню сообщения после долгого нажатия; интеграции аудиоплеера и профиля работают в режиме best-effort и зависят от версии клиента.
- Запрос состояния плеера с телефона: название медиа, текущая позиция, длительность и состояние паузы.
- Полученные файлы сохраняются во временную папку AERIS в Windows.
- Браузерная загрузка на ПК позволяет отправить локальный файл с Android без Telegram (порт 45873).

### Состояние разработки

AERIS 1.0.0 — первая публичная версия проекта. Она поддерживает передачу медиа с Android на Windows, воспроизведение и удалённое управление по локальной сети. Android-компонент — только плагин для Telegram; отдельного Android-приложения и APK нет. Плагин запрашивает уже загруженный файл через FileLoader активного клиента, поэтому медиа должно быть доступно локально. Сама передача идёт напрямую по локальной сети и не проходит через серверы Telegram. Если Telegram заблокирован, но файл уже кэширован и устройства видят друг друга по LAN, передача всё равно может работать. Если Telegram не может выдать файл, можно воспользоваться браузерной загрузкой и выбрать его из памяти телефона.

Подтверждённая точка входа в Telegram — пункт «Воспроизвести через AERIS» в меню сообщения после долгого нажатия. В плагине также есть best-effort интеграции с меню аудиоплеера и действиями профильной музыки; они зависят от версии AyuGram/exteraGram. Пункт в меню «три точки» непосредственно у отдельного видео пока не реализован. AERIS Desktop дополнительно поднимает браузерную страницу http://<IP-компьютера>:45873, откуда на телефоне можно выбрать локальный файл. Windows-плеер открывается в отдельном окне mpv. Возобновление оборванной передачи, шифрование транспорта, интерфейс управления устройствами и пункт меню у видео не входят в текущий кандидат.

### Безопасность

Сейчас протокол использует незашифрованный WebSocket (ws://) в частной локальной сети. Приёмник отклоняет подключения с IP-адресов, не относящихся к частным, loopback или link-local диапазонам; токены сопряжения хранятся локально в плагине и Desktop. Это дополнительная защита, а не шифрование: используй AERIS только в доверенной домашней сети и не открывай TCP-порты 45872 и 45873 в интернет.

### Структура репозитория

AERIS/
- desktop/ — приёмник и контроллер плеера для Windows на Rust
- plugin/ — Python-плагин AyuGram / exteraGram
- protocol/ — спецификация общего протокола
- licenses/ — лицензии сторонних компонентов
- .github/workflows/ — CI и сборки релизов

### Релизы

Сборка Windows версии 1.0.0 публикуется в GitHub Releases. Android-компонент распространяется как исходный код/пакет плагина для совместимых клиентов:
- **Windows:** портативный ZIP с AERIS Desktop и движком mpv.
- **Android:** исходный код плагина AERIS для совместимых клиентов AyuGram / exteraGram.

### Участие в проекте

Приветствуются сообщения об ошибках, предложения и вклад в разработку. При сообщении о проблеме укажи версию Windows, версию Android-клиента и соответствующие журналы AERIS.
