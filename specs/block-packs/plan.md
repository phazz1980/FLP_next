# План: наборы блоков

- `flp-core/src/pack.rs` (новый): `PackInfo{name,version,description,authors,contacts,license}`; `contact_url(&str)->String` (почта → `mailto:`, без схемы → `https://`) ↔ `pack.toml`; чтение/запись; `read_pack_file`/`export_pack` (ZIP `.flpblocks`: `pack.toml` + `<тип>/…` для каждого блока); `IncomingPack{info, blocks: Vec<Incoming>}`; `install_pack` (замена папки набора целиком, проверка конфликтов типов).
- `library.rs`: при загрузке пользовательской папки с `pack.toml` в корне — набор: группа блока = имя папки набора, `Library::packs()` для списка и версий; `info.version` блока не используется внутри набора (берётся версия набора).
- Версия: `bump` при сохранении блока набора поднимает `pack.toml.version`, а не `block.toml`.
- `blockfile.rs`: убрать формат `.flpblock` и `Conflict::User(version)`; общий код чтения/записи файлов блока переиспользуется в `pack.rs`. Из `block.toml` убрать `version` и автоповышение (`library.rs` ~l.736).
- `flp-editor`: `userlib.rs` — кнопки «Новый набор», «Экспорт набора», «Перенести в набор»; `blockedit.rs` — выбор набора при создании; `blockio.rs` — окно импорта показывает наборы; `assoc.rs` — регистрация `.flpblocks`; `app.rs` — аргумент командной строки и drop `.flpblocks`.
- `flp-cli`: `flp block export-pack/import` расширить на наборы.
- Риски: перенос блока между наборами меняет путь папки (группу) — проекты ссылаются по типу, не по пути, значит безопасно; в архитектурном файле обновить раздел про библиотеку блоков.
