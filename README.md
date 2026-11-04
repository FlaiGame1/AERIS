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
- Received files are stored under the Windows temporary AERIS folder.

### Development status

AERIS is in early development. The Android plugin currently reads media from the Telegram client's local cache, so the media must be downloaded in Telegram before sending. The actual transfer from phone to PC is local and does not pass through Telegram servers. If Telegram is blocked but the file is already cached on the phone and both devices can communicate over the local network, transfer can still work.

A standalone Android file picker / sender is not implemented yet. Video is played in a separate mpv window; an embedded player surface, transfer resume, transport encryption and a device-management screen remain future work.

### Security

The current protocol uses unencrypted WebSocket (ws://) on the private LAN. Pairing tokens are stored locally by the plugin and Desktop. Use only a trusted home network and never expose TCP port 45872 to the public internet.

### Repository structure

AERIS/
- desktop/ — Rust Windows receiver and player controller
- plugin/ — Python AyuGram / exteraGram plugin
- protocol/ — shared protocol specification
- docs/ — architecture and developer documentation
- licenses/ — third-party license notices
- .github/workflows/ — CI and release builds

### Releases

Official packages are published through GitHub Releases:
- **Windows:** portable ZIP with AERIS Desktop and the bundled mpv runtime.
- **Android:** AERIS plugin package/source compatible with the documented AyuGram / exteraGram SDK versions.

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
- Полученные файлы сохраняются во временную папку AERIS в Windows.

### Состояние разработки

AERIS всё ещё находится на ранней стадии. Android-плагин пока получает файлы из локального кэша Telegram, поэтому медиа сначала нужно скачать в Telegram. Но сама передача с телефона на компьютер идёт напрямую по локальной сети и не проходит через серверы Telegram. Если Telegram заблокирован, но файл уже находится в кэше телефона и устройства видят друг друга по локальной сети, передача всё равно может работать.

Отдельный Android-отправитель с выбором файлов пока не реализован. Видео открывается в отдельном окне mpv, а не во встроенной поверхности AERIS. Встроенный видеоплеер, возобновление прерванной передачи, шифрование соединения и экран управления подключёнными устройствами остаются дальнейшими задачами.

### Безопасность

Сейчас протокол использует незашифрованный WebSocket (ws://) в частной локальной сети. Токены сопряжения хранятся локально в плагине и Desktop. Используй AERIS только в доверенной домашней сети и не открывай TCP-порт 45872 в интернет.

### Структура репозитория

AERIS/
- desktop/ — приёмник и контроллер плеера для Windows на Rust
- plugin/ — Python-плагин AyuGram / exteraGram
- protocol/ — спецификация общего протокола
- docs/ — архитектура и документация разработчика
- licenses/ — лицензии сторонних компонентов
- .github/workflows/ — CI и сборки релизов

### Релизы

Официальные сборки публикуются в GitHub Releases:
- **Windows:** портативный ZIP с AERIS Desktop и движком mpv.
- **Android:** пакет/исходный файл плагина AERIS для совместимых версий AyuGram / exteraGram SDK.

### Участие в проекте

Приветствуются сообщения об ошибках, предложения и вклад в разработку. При сообщении о проблеме укажи версию Windows, версию Android-клиента и соответствующие журналы AERIS.
