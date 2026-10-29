# Northstar Launcher

Northstar is a Linux-first, open-source Minecraft launcher built by adapting the NoRisk Client launcher. The upstream Java installation, profile, loader, asset, process, and content-management systems are retained where they fit. This repository is an early fork; it is **not yet a Northstar 1.0 release**.

## Current status

| Area | Current state |
| --- | --- |
| Classic Minecraft Java | Reuses the upstream Java launcher, profiles, downloads, version metadata, and Fabric, Forge, NeoForge, and Quilt integration. |
| Modrinth | Reuses upstream search, project/version details, profile installation, content updates, and installed-content workflows. CurseForge support is also present upstream. |
| Account sign-in | An Ely.by Yggdrasil client and Linux Secret Service storage backend exist, but the account screen and Java launch pipeline still use the upstream Microsoft/Minecraft flow. Ely.by is not yet a user-facing sign-in option. |
| Offline accounts and custom skins | Rust support exists for username validation, deterministic launcher-local UUIDv8 identities, metadata persistence, and PNG skin file validation. The account screen, skin preview, and Java launch session are not wired to it yet. |
| Horizon native engine | A launch adapter is present for a user-selected `horizon-client` binary and prepared resource directory. It exposes documented world/render-distance options and process status/termination. Installation, resource preparation, updates, multiplayer authentication, and online account bridging are not integrated. |
| Linux packages | Tauri configuration includes Linux bundlers. A local AppImage build currently depends on downloading AppImageKit's `AppRun`; network DNS failure can interrupt packaging. |

The current frontend build is not evidence that pending authentication, Horizon installation, or account bridging is complete. Do not use offline identities to access authenticated online servers.

## Upstream reuse and attribution

The base is [NoRiskClient/noriskclient-launcher](https://github.com/NoRiskClient/noriskclient-launcher), GPL-3.0. Its Java launch pipeline (`src-tauri/src/minecraft`), profile and process commands (`src-tauri/src/commands/profile_command.rs`, `minecraft_command.rs`), Java discovery (`src-tauri/src/utils/java_detector.rs`), Modrinth integration (`src-tauri/src/integrations/modrinth.rs`, `src-tauri/src/commands/modrinth_commands.rs`), and related React UI are upstream-derived and remain attributed to their authors. Changes in this fork are distributed under the applicable GPL-3.0 terms; see [LICENSE](LICENSE) and retain upstream notices when redistributing.

## Requirements

- Linux desktop with a supported WebKitGTK runtime and graphics drivers.
- Node.js 18 or later, Yarn Classic (the repository uses `yarn.lock`), and stable Rust.
- Tauri Linux build dependencies: WebKitGTK 4.1 development files, `libayatana-appindicator3-dev`, `librsvg2-dev`, and `patchelf`. AppImage bundling may require network access to fetch AppImageKit's runtime.
- Java for Minecraft versions/loaders that need a local Java runtime. The launcher includes Java detection; install a runtime appropriate to the selected Minecraft version.

## Build from source

```bash
git clone --recurse-submodules <your-Northstar-repository-url> northstar-launcher
cd northstar-launcher
yarn install --frozen-lockfile
yarn tauri dev
```

Build Linux bundles with:

```bash
yarn tauri build --bundles deb,appimage
```

The Tauri configuration is in `src-tauri/tauri.conf.json`; compiled artifacts are written under `src-tauri/target/release/bundle/`. The repository currently has no Flatpak manifest or verified Flatpak build, and it does not publish Northstar release artifacts yet.

## Development checks

```bash
yarn build
cd src-tauri
cargo test --workspace
cargo clippy --workspace --all-targets
```

The frontend package has no standalone lint or test script at present. `cargo fmt --all -- --check` currently reports formatting differences in the upstream tree; format changes should be reviewed carefully instead of reformatting unrelated upstream files wholesale.

## Configuration and data

The launcher uses its existing Tauri/NoRisk application data directories and per-profile game directories. Existing configuration controls the launcher data path and profile settings. The current upstream Microsoft account store writes credentials to `accounts.json`; this fork has not yet migrated those secrets into OS-backed secure storage. Treat the local account data directory as sensitive and do not include it in bug reports.

Mod files and packs are downloaded as content and are not intentionally executed by the launcher as standalone programs. Review third-party content and its permissions before use.

## Troubleshooting

- **AppImage packaging cannot download `AppRun`:** check DNS/network access and retry `yarn tauri build --bundles appimage`. A release build must be completed in a network-enabled environment.
- **Java not found or incompatible:** install the Java runtime required by the selected Minecraft version and select/check it in launcher settings.
- **Linux window or graphics issues:** update the system WebKitGTK and graphics stack; try the session type (Wayland/X11) supported by the installed desktop drivers.
- **Account or profile problems:** attach launcher logs only after checking them for personal paths and account information. Never share tokens or the contents of `accounts.json`.

## Security

Tauri commands that touch files or spawn processes run in Rust. Authentication data must not be sent to the React layer or printed in logs. The existing upstream authentication and credential-storage design is still present and must be replaced or hardened before a Northstar 1.0 release. No Ely.by or Horizon online-authentication compatibility is claimed.

## Roadmap

1. Connect the offline/Ely.by backend to the account screen and Java launch session; add authlib-injector acquisition and integrity checks for Ely.by. Add OS keyring backends beyond Linux or disable Ely.by where secure storage is unavailable.
2. Complete Horizon installation, resource preparation, updates, cross-platform packaging, and process cleanup. Keep online authentication disabled until the engine documents and supports a compatible protocol.
3. Add and verify Linux CI packaging, including a Flatpak workflow, then review the security configuration and migration behavior.
4. Run the complete test/build matrix, document supported combinations, and only then prepare a 1.0 release.

## Contributing

Keep changes small, preserve upstream behavior and notices, and state clearly whether code is upstream-derived or new. Include reproducible commands and actual results for checks you run. Do not commit game assets, account data, or authentication tokens.

## Українською

### Огляд

Northstar — лаунчер Minecraft із пріоритетом Linux, створений на основі лаунчера NoRisk Client. Ми зберігаємо сумісні системи upstream: встановлення Java Minecraft, профілі, завантаження ресурсів і модлоадери. Це рання версія форка, **ще не реліз Northstar 1.0**.

### Поточний стан і сумісність

- **Minecraft Java:** використовується наявний upstream-конвеєр запуску, профілі, метадані версій і підтримка Fabric, Forge, NeoForge та Quilt.
- **Modrinth:** використовуються пошук, картки проєктів і версій, встановлення в профіль та оновлення контенту з upstream. Також збережена інтеграція CurseForge.
- **Облікові записи:** є Rust-клієнт Ely.by Yggdrasil і збереження токенів у Linux Secret Service, але екран акаунтів і Java-запуск досі використовують upstream-вхід Microsoft/Minecraft. Ely.by ще не доступний як варіант входу в інтерфейсі.
- **Офлайн-профілі й власні скіни:** є Rust-основа для перевірки імені, стабільного локального UUIDv8, збереження метаданих і перевірки PNG-скінів. Екран акаунтів, попередній перегляд скіна та Java-сесія ще не під'єднані.
- **Horizon:** є адаптер запуску вибраного користувачем `horizon-client` із підготовленими ресурсами, назвою світу й дальністю промальовування; доступні стан процесу та його зупинка. Встановлення рушія, підготовка ресурсів, оновлення й автентифікація онлайн-акаунтів ще не інтегровані.
- **Linux-пакети:** конфігурація Tauri містить Linux-бандлери. Локальна збірка AppImage завантажує `AppRun` з AppImageKit, тож проблеми DNS можуть її зупинити.

Успішна збірка інтерфейсу не означає, що Ely.by, офлайн-вхід або Horizon уже працюють. Офлайн-ідентичність не дає доступу до серверів, які вимагають автентифікований акаунт.

### Повторне використання upstream і ліцензія

Базовий репозиторій: [NoRiskClient/noriskclient-launcher](https://github.com/NoRiskClient/noriskclient-launcher), ліцензія GPL-3.0. Java-конвеєр (`src-tauri/src/minecraft`), команди профілів і процесів, визначення Java, інтеграція Modrinth та пов'язаний React-інтерфейс походять з upstream і залишаються його авторським кодом. Під час поширення зберігайте повідомлення про авторські права й умови GPL-3.0; див. [LICENSE](LICENSE).

### Вимоги

- Linux із підтримуваним WebKitGTK та графічними драйверами.
- Node.js 18+, Yarn Classic (у репозиторії є `yarn.lock`) і стабільний Rust.
- Для Linux-збірки Tauri потрібні WebKitGTK 4.1 development files, `libayatana-appindicator3-dev`, `librsvg2-dev` і `patchelf`. Для AppImage може знадобитися мережа, щоб завантажити runtime AppImageKit.
- Java потрібної версії для вибраної версії Minecraft і модлоадера. Лаунчер уміє шукати встановлену Java.

### Збірка з вихідного коду

```bash
git clone --recurse-submodules <URL-вашого-репозиторію-Northstar> northstar-launcher
cd northstar-launcher
yarn install --frozen-lockfile
yarn tauri dev
```

Створення Linux-пакетів:

```bash
yarn tauri build --bundles deb,appimage
```

Налаштування Tauri розташоване в `src-tauri/tauri.conf.json`, артефакти — у `src-tauri/target/release/bundle/`. У репозиторії ще немає маніфесту Flatpak або перевіреної збірки Flatpak; релізи Northstar поки не публікуються.

### Перевірки розробки

```bash
yarn build
cd src-tauri
cargo test --workspace
cargo clippy --workspace --all-targets
```

Окремих скриптів frontend lint і test поки немає. `cargo fmt --all -- --check` наразі знаходить розбіжності форматування в upstream-коді; перед форматуванням великих файлів переглядайте зміни, щоб не переписати несуміжний код.

### Налаштування, безпека й діагностика

Лаунчер використовує наявні каталоги даних Tauri/NoRisk та окремі каталоги профілів. Поточний upstream-запис акаунтів зберігає облікові дані у `accounts.json`; перенесення токенів у захищене сховище ОС ще не виконано. Вважайте каталог даних чутливим і не додавайте його до звітів про помилки. Не надсилайте токени чи `accounts.json`.

Якщо AppImage-збірка не може завантажити `AppRun`, перевірте DNS/мережу та повторіть `yarn tauri build --bundles appimage`. Якщо Java відсутня або несумісна, встановіть відповідний runtime і перевірте його в налаштуваннях. Проблеми вікна/графіки на Linux можуть залежати від WebKitGTK, драйверів і сесії Wayland/X11.

### План

1. Під'єднати offline/Ely.by backend до екрана акаунтів і Java-сесії; додати отримання та перевірку цілісності authlib-injector для Ely.by. Додати системні сховища ключів поза Linux або вимикати Ely.by там, де безпечне сховище відсутнє.
2. Завершити встановлення Horizon, підготовку ресурсів, оновлення, пакування й коректне завершення процесу. Не вмикати онлайн-автентифікацію, доки рушій не документує сумісний протокол.
3. Налаштувати й перевірити Linux CI та Flatpak, перевірити безпеку й міграції.
4. Прогнати повні тести й збірки, задокументувати сумісні конфігурації та лише після цього готувати реліз 1.0.

### Внесок

Робіть невеликі зміни, зберігайте сумісність і upstream-повідомлення, чітко вказуйте походження коду та наводьте фактичні результати перевірок. Не додавайте ігрові ресурси, дані акаунтів чи токени.
