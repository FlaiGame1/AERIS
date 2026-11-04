# AERIS

**Open-source media casting ecosystem / Открытая экосистема трансляции медиа**

[English](#english) · [Русский](#русский)

---

## English

AERIS is an open-source project for sending media from Android to a Windows PC over a local network — without relying on Google Chromecast or Telegram servers for the actual transfer.

### Components

- **AERIS Desktop** — a Windows receiver and playback controller, written in Rust.
- **AERIS Plugin** — a Python plugin for AyuGram / exteraGram that sends cached Telegram media and provides remote playback controls.
- **AERIS Protocol** — a shared WebSocket protocol for pairing, media transfer and playback commands.

### Current features

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

AERIS is a working first-version candidate, not yet an official release. The Android side is a Telegram plugin only; there is no separate Android application. The plugin asks the active Telegram client's FileLoader for an already downloaded media file, so media must be available locally before sending. The transfer itself is direct over the local network and does not pass through Telegram servers. If Telegram is blocked but the file is already cached and both devices can communicate over the LAN, the transfer can still work. When Telegram cannot provide the file, the browser uploader can send a file selected from Android storage.

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

Official packages are published through GitHub Releases:
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

### Что уже реализовано

- Прямая передача с телефона на ПК через локальную Wi-Fi-сеть.
- Первичное сопряжение по короткому коду и запоминание устройства по токену.
- Передача блоками по 1 МиБ с проверкой целостности SHA-256.
- Воспроизведение аудио и видео через движок mpv, включённый в поставку.
- Управление с телефона: пауза/продолжение, остановка, перемотка назад/вперёд и изменение громкости.
- Действия плагина в меню сообщений Telegram и меню действий профиля (получение самого профильного трека зависит от того, предоставляет ли клиент его плагинам).
- Запрос состояния плеера с телефона: название медиа, текущая позиция, длительность и состояние паузы.
- Полученные файлы сохраняются во временную папку AERIS в Windows.

### Состояние разработки

AERIS всё ещё находится на ранней стадии. Android-плагин пока получает файлы из локального кэша Telegram, поэтому медиа сначала нужно скачать в Telegram. Но сама передача с телефона на компьютер идёт напрямую по локальной сети и не проходит через серверы Telegram. Если Telegram заблокирован, но файл уже находится в кэше телефона и устройства видят друг друга по локальной сети, передача всё равно может работать.

Пункт меню профиля теперь проверяет поле Telegram `saved_music` и пытается получить локальный файл документа через `FileLoader`. Также добавлен защищённый низкоуровневый Java-хук: он пытается добавить «Воспроизвести через AERIS» непосредственно в меню «три точки» встроенного `AudioPlayerAlert`, расширяя `ItemOptions`. Этот хук зависит от внутренних имён классов и методов конкретной версии Telegram, поэтому его ещё нужно проверить на телефоне с твоей сборкой AyuGram/exteraGram. Плагин передаёт медиа, уже доступное в локальном кэше Telegram. Если Telegram недоступен, открой на телефоне в браузере http://<IP-компьютера>:45873, введи короткий код сопряжения, выбери локальный аудио- или видеофайл и отправь его напрямую на ПК. Видео пока воспроизводится в отдельном окне mpv, а не во встроенной поверхности AERIS. Возобновление прерванной передачи, шифрование соединения и экран управления подключёнными устройствами остаются дальнейшими задачами.

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

Официальные сборки публикуются в GitHub Releases:
- **Windows:** портативный ZIP с AERIS Desktop и движком mpv.
- **Android:** исходный код плагина AERIS для совместимых клиентов AyuGram / exteraGram.

### Участие в проекте

Приветствуются сообщения об ошибках, предложения и вклад в разработку. При сообщении о проблеме укажи версию Windows, версию Android-клиента и соответствующие журналы AERIS.
