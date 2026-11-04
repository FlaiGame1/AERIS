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
- Send supported media links and metadata (title, thumbnail, MIME type).
- Play, pause, stop, seek and control volume from Telegram.
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

AERIS is in early development. The previous NEXUS CAST prototype is the starting point for the desktop receiver; its experimental Google Cast implementation will be replaced by the native AERIS protocol. The Android plugin and protocol are being developed alongside it.

Do not consider development builds stable or secure for use on untrusted networks.

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
- Управление воспроизведением прямо из Telegram: запуск, пауза, остановка, перемотка и громкость.
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

AERIS находится на ранней стадии разработки. В качестве основы приложения для Windows используется предыдущий прототип NEXUS CAST. Его экспериментальная реализация Google Cast будет заменена собственным протоколом AERIS. Плагин для Android и протокол будут разрабатываться параллельно.

Тестовые сборки не следует считать стабильными или безопасными для использования в недоверенных сетях.

### Релизы

В официальных релизах будут публиковаться:
- **Windows:** ZIP-архив с AERIS Desktop и необходимыми файлами.
- **Android:** пакет/исходный файл плагина AERIS для документированных версий SDK AyuGram / exteraGram.

### Участие в проекте

Приветствуются сообщения об ошибках, предложения и вклад в разработку. При описании проблем с подключением указывайте версию Windows, версию Android-клиента и соответствующие журналы.
