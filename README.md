# Deb2gpkg (d2g)

## 🇬🇧 English

**Deb2GPKG** is a modern utility designed to convert Debian packages (`.deb`) into native G OS packages (`.gpkg`)[cite: 7, 8, 9]. It features an intuitive graphical user interface and a robust, autonomous backend written in Rust[cite: 7, 8, 10].

---

## 🚀 Features

* **Native Debian Unpacking:** Unpacks `.deb` files natively using the `ar` archive format and supports multiple compression standards including gzip, xz, and zstd for `control.tar` and `data.tar` streams[cite: 5].
* **Control Parsing & Manifest Generation:** Parses the Debian control file to extract metadata such as package name, version, architecture, maintainer, description, and dependencies, compiling them into a `manifest.json` file[cite: 2].
* **Hook Management:** Automatically detects and sets up standard installation hooks (`preinst`, `postinst`, `prerm`, `postrm`) into the package staging structure[cite: 5].
* **ZSTD Archive Builder:** Compiles the final `.gpkg` archive using ZSTD compression, bundling the manifest, hooks, and payload data[cite: 6].
* **Slint Graphical Interface:** Provides an interactive GUI (Liquid Glass Edition) built with Slint and powered by Tokio for asynchronous operations and native file dialogs (`rfd`)[cite: 3, 7, 8, 10].

---

## 🛠 Tech Stack & Dependencies

The project relies on the following core libraries and frameworks:
* **Language:** Rust (Edition 2021)[cite: 7, 8]
* **GUI Framework:** Slint (`1.8`)[cite: 7, 8]
* **Async Runtime:** Tokio (`1.0`)[cite: 7, 8]
* **Serialization:** Serde and Serde JSON[cite: 7, 8]
* **Archive & Compression:** `ar`, `tar`, `flate2`, `xz2`, `zstd`[cite: 7, 8]
* **File Dialogs:** `rfd`[cite: 7, 8]

---

## 🇷🇺 Русский

**Deb2GPKG** — это современная утилита, предназначенная для конвертации пакетов Debian (`.deb`) в нативные пакеты G OS (`.gpkg`)[cite: 7, 8, 9]. Она оснащена интуитивным графическим интерфейсом и надежной автономной частью, написанной на Rust[cite: 7, 8, 10].

---

## 🚀 Основные возможности

* **Нативная распаковка Debian:** Распаковывает `.deb` файлы с использованием формата архива `ar` и поддерживает различные стандарты сжатия, включая gzip, xz и zstd для потоков `control.tar` и `data.tar`[cite: 5].
* **Парсинг control и генерация манифеста:** Анализирует control-файл Debian для извлечения метаданных, таких как имя пакета, версия, архитектура, мейнтейнер, описание и зависимости, объединяя их в файл `manifest.json`[cite: 2].
* **Управление хуками:** Автоматически обнаруживает и настраивает стандартные хуки установки (`preinst`, `postinst`, `prerm`, `postrm`) в структуре пакета[cite: 5].
* **Сборщик архивов ZSTD:** Собирает финальный `.gpkg` архив с использованием сжатия ZSTD, упаковывая манифест, хуки и полезную нагрузку[cite: 6].
* **Графический интерфейс Slint:** Предоставляет интерактивный GUI (Liquid Glass Edition), созданный на Slint и работающий на базе Tokio для асинхронных операций и нативных диалоговых окон файлов (`rfd`)[cite: 3, 7, 8, 10].

---

## 🛠 Стек технологий и зависимости

Проект использует следующие ключевые библиотеки и фреймворки:
* **Язык:** Rust (Edition 2021)[cite: 7, 8]
* **Фреймворк GUI:** Slint (`1.8`)[cite: 7, 8]
* **Асинхронная среда:** Tokio (`1.0`)[cite: 7, 8]
* **Сериализация:** Serde и Serde JSON[cite: 7, 8]
* **Архивы и сжатие:** `ar`, `tar`, `flate2`, `xz2`, `zstd`[cite: 7, 8]
* **Диалоги файлов:** `rfd`[cite: 7, 8]

---
