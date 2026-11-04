# AERIS

**Open-source media casting ecosystem / Открытая экосистема трансляции медиа**

[English](#english) · [Русский](#русский)

---

## English

AERIS is an open-source project for sending media from Android to a Windows PC over a local network — without relying on Google Chromecast.

The project has two components in one repository:

- **AERIS Desktop** — a Windows receiver and media player, written in Rust.
- **AERIS Plugin** — a Python plugin for AyuGram / exteraGram, designed to send media from Telegram and control playback.

The components communicate through the custom **AERIS Protocol**.

### Planned features

- Pair an Android device with a Windows PC.
- Transfer Telegram media files directly over the local network in verified chunks (up to 512 MiB).
- Play supported audio formats in the native AERIS audio engine; open video and other formats with the Windows default application.
- Control audio playback remotely from the Android plugin (play, pause, stop, seek and volume).
- Display playback status on the phone.
- Portable Windows package with required runtime files.
- Plugin and desktop app distributed together through GitHub Releases.

### Repository structure

```text
AERIS/
├── desktop/          # Rust Windows receiver
├── plugin/           # Python AyuGram / exteraGram plugin
├── protocol/         # Shared protocol specification
├── docs/             # Architecture and developer documentation
├── licenses/         # Third-party license notices
└── .github/workflows # CI and release builds
```

### Development status

AERIS is in early development. The Windows app now starts the first native AERIS Protocol v1 WebSocket receiver with a temporary pairing code. The plugin has connection settings, a persistent paired WebSocket session, a Telegram message context-menu action and remote audio controls. It attempts to resolve media from Telegram's local cache and transfers it in 1 MiB chunks. Desktop verifies file size and SHA-256, plays supported audio formats through the native AERIS audio engine, and opens video/other formats with the Windows default application. Media that Telegram has not downloaded yet still needs to be downloaded in Telegram first. Native in-app video playback, Telegram's built-in media-player overflow-menu integration, encrypted transport and persistent pairing credentials are not implemented yet. The temporary pairing code remains valid for the lifetime of the running desktop receiver so a dropped LAN connection can reconnect; restart the receiver to rotate the code.

The old Google Cast prototype remains in the repository as inactive reference code, but it is no longer used by the active desktop application. Development builds use unencrypted WebSocket and must only be tested on a trusted private network.

### Releases

Official releases will include:
- **Windows:** portable ZIP containing AERIS Desktop and its required runtime files.
- **Android:** AERIS plugin package/source compatible with the documented AyuGram / exteraGram SDK versions.

### Contributing

Issues, bug reports and contributions are welcome. Please include your Windows version, Android client version and relevant logs when reporting connection problems.

---

## Русский

AERIS — открытый проект для передачи видео и другого медиаконтента с Android на компьютер с Windows через локальную сеть — без зависимости от Google Chromecast.

В одном репозитории находятся два компонента:

- **AERIS Desktop** — приложение-приёмник и медиаплеер для Windows, написанное на Rust.
- **AERIS Plugin** — Python-плагин для AyuGram / exteraGram, предназначенный для отправки медиа из Telegram и управления воспроизведением.

Компоненты взаимодействуют через собственный **протокол AERIS**.

### Планируемые возможности

- Подключение Android-устройства к компьютеру.
- Передача поддерживаемых ссылок на медиа и метаданных: названия, обложки и MIME-типа.
- Управление аудиовоспроизведением с телефона: запуск, пауза, остановка, перемотка и громкость.
- Отображение состояния воспроизведения на телефоне.
- Портативная версия приложения для Windows со всеми необходимыми файлами.
- Публикация приложения и плагина вместе в GitHub Releases.

### Структура репозитория

```text
AERIS/
├── desktop/          # Приёмник для Windows на Rust
├── plugin/           # Python-плагин AyuGram / exteraGram
├── protocol/         # Спецификация общего протокола
├── docs/             # Архитектура и документация разработчика
├── licenses/         # Лицензии сторонних компонентов
└── .github/workflows # CI и сборки релизов
```

### Состояние разработки

AERIS находится на ранней стадии разработки. Приложение для Windows уже запускает первую реализацию собственного протокола AERIS v1 на WebSocket с временным кодом сопряжения. В плагине появились настройки подключения и проверка сопряжения/ping. Плагин добавляет действие в контекстное меню сообщения Telegram, пытается получить файл из локального кэша и передаёт его блоками по 1 МиБ. Desktop проверяет размер и SHA-256, поддерживаемые аудиоформаты воспроизводит встроенным аудиодвижком AERIS, а видео и другие форматы открывает стандартным приложением Windows. С телефона доступны команды воспроизведения, паузы, остановки, перемотки, громкости и запрос состояния плеера в настройках плагина. Медиа, которое Telegram ещё не скачал, сначала нужно загрузить в самом Telegram. Встроенный видеоплеер, интеграция в меню трёх точек штатного Telegram-плеера, шифрование и постоянные учётные данные сопряжения пока не реализованы. Код сопряжения действует, пока запущен Desktop, чтобы плагин мог переподключиться после обрыва локальной сети; перезапуск Desktop меняет код.

Старый прототип Google Cast оставлен в репозитории как справочный код, но активное приложение его больше не использует. Тестовая версия использует незашифрованный WebSocket и предназначена только для доверенной домашней сети.

### Релизы

В официальных релизах будут публиковаться:
- **Windows:** ZIP-архив с AERIS Desktop и необходимыми файлами.
- **Android:** пакет/исходный файл плагина AERIS для документированных версий SDK AyuGram / exteraGram.

### Участие в проекте

Приветствуются сообщения об ошибках, предложения и вклад в разработку. При описании проблем с подключением указывайте версию Windows, версию Android-клиента и соответствующие журналы.
