# Checkpoint

[English](README.md) · **Tiếng Việt**

[![Latest release](https://img.shields.io/github/v/release/fr0stf0x/checkpoint)](https://github.com/fr0stf0x/checkpoint/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![Downloads](https://img.shields.io/github/downloads/fr0stf0x/checkpoint/total)](https://github.com/fr0stf0x/checkpoint/releases)
[![Ko-fi](https://img.shields.io/badge/Ko--fi-fr0stf0x-FF5E5B?logo=kofi&logoColor=white)](https://ko-fi.com/fr0stf0x)

Backup và restore save game Steam giữa các máy của bạn (Windows và Linux, kể cả Bazzite) qua chính Google Drive của bạn. Dùng được khi Steam chạy offline mode, lúc Steam Cloud không hoạt động.

Tải về: [bản mới nhất](https://github.com/fr0stf0x/checkpoint/releases/latest) · Trang web và hướng dẫn: xem thư mục `site/`

## Tính năng

- Liệt kê game Steam đã cài và các tài khoản Steam trên máy, kèm ảnh game và avatar tài khoản lấy từ bộ nhớ đệm của Steam trên máy (chạy được khi offline). Game hiện dạng lưới ảnh bìa hoặc danh sách chi tiết (nút chuyển cạnh ô tìm kiếm), giao diện tối có icon.
- Tìm file save bằng [ludusavi-manifest](https://github.com/mtkennerly/ludusavi-manifest) (dữ liệu PCGamingWiki, khoảng 20.000 game), cộng thêm thư mục `userdata/<account>/<appid>/remote` của Steam Cloud.
- Backup và restore từng game, nhiều game hoặc toàn bộ cùng lúc. Rê chuột vào game (hoặc di chuyển tới bằng tay cầm) rồi tick ô để chọn; nút trên đầu đổi thành **Backup N game / Restore N game**. Màn hình xem trước liệt kê game nào chạy, game nào bỏ qua, cho bỏ tick từng game, và hiện dung lượng từng game cùng tổng cộng. Thẻ game và danh sách hiện dung lượng save trên máy và bản backup trên cloud. Trong lúc backup hoặc restore, app hiện danh sách từng file save (tên file và thư mục rút gọn), file đang nén hoặc đang ghi có vòng xoay, xong thì có dấu tick, kèm dòng tải lên hoặc tải về có phần trăm.
- So sánh bằng hash nội dung, không dựa vào ngày sửa file. Phân biệt "local mới hơn", "cloud mới hơn" và xung đột thật, cảnh báo trước khi nút nào đó ghi đè tiến trình mới hơn.
- Giữ 10 bản gần nhất mỗi game trên cloud. Khi một lần backup ghi đè tiến trình mà máy này không có (xung đột, hoặc **Vẫn backup** khi bản cloud mới hơn, kể cả từ launch option), bản cloud bị thay được đánh dấu **Đang giữ**: không tính vào 10 bản và không bao giờ tự bị xóa. Bấm **Bỏ giữ** trong Lịch sử để thả nó ra.
- Hoàn tác restore: trước khi restore ghi đè save, save trên máy được giữ lại (10 bản gần nhất mỗi game). Bấm **Hoàn tác** trong thông báo sau khi restore, hoặc mở **⋯ → Save cũ trên máy** của game để xem từng bản (chơi lần cuối lúc nào, bị bản nào ghi đè, dung lượng) và quay lại bằng 1 click. Khi quay lại, save hiện tại cũng được giữ, nên bạn đổi ý tiếp được.
- Không cho backup hoặc restore khi game đang chạy. Đánh dấu game có save gắn với SteamID.
- Save tạo trên Windows restore được vào Proton prefix trên Linux và ngược lại.
- Chuyển save của một game từ tài khoản Steam này sang tài khoản khác (ví dụ khi phải đổi tài khoản): copy thư mục save sang tài khoản mới, sửa `steam_autocloud.vdf`, tùy chọn thay SteamID bên trong file save, rồi để bạn thử trong game và **Giữ** hoặc **Hoàn tác** bằng 1 click. File của tài khoản cũ không bao giờ bị đụng tới. Restore cũng ghi được bản backup vào tài khoản khác.
- **Tài khoản liên kết** (Tài khoản Steam → Liên kết tài khoản…): đánh dấu nhiều tài khoản Steam là cùng một người chơi, ví dụ thư viện chia sẻ qua Steam Family mà máy này chơi bằng tài khoản A, máy kia bằng tài khoản B. Save gắn tài khoản khi đó đi theo người chơi: mỗi máy chỉ backup tài khoản đang đăng nhập trên máy đó và restore vào chính nó, kể cả qua launch option, không cần chuyển tay. Tên thư mục và `steam_autocloud.vdf` được sửa lại; SteamID đọc được bên trong file save được thay (thử nghiệm); save mã hóa có thể không chạy trên tài khoản khác nên cần thử trong game. Liên kết lưu cùng các bản backup nên mọi máy đều dùng chung.
- Nút **⋯** của mỗi game mở chi tiết game (thư mục save, bản trên máy và trên cloud) kèm **Xóa bản trên cloud** và **Báo lỗi game này**. **Cài đặt → Nơi lưu backup → Xóa toàn bộ dữ liệu trên cloud** xóa mọi bản backup một lần. Cả hai đều hỏi xác nhận và không bao giờ đụng tới save trên các máy của bạn.
- **Thêm Checkpoint vào Steam** (Cài đặt → Máy này) thêm app vào thư viện Steam dưới dạng game ngoài Steam, để mở được từ thư viện và từ Gaming Mode trên Bazzite / SteamOS. Thoát Steam trước; danh sách cũ của Steam được giữ lại thành `shortcuts.vdf.checkpoint-bak`. Gaming Mode không có nút đóng cửa sổ, nên sidebar có nút **Thoát** ngay dưới Báo lỗi / Góp ý.
- Tự đồng bộ từ Steam: copy tùy chọn khởi chạy trong hộp thoại **⋯** của game (`"<đường dẫn Checkpoint>" run --appid <appid> -- %command%`) rồi dán vào **Properties → Launch options** của game. Trước khi game chạy, Checkpoint restore save trên cloud nếu cloud mới hơn; sau khi bạn thoát (và không còn gì chạy từ thư mục game), app backup nếu save trên máy mới hơn. Nếu lúc mở game save bị xung đột (cả hai bên đều đổi, hoặc máy này chưa đồng bộ lần nào), một cửa sổ Checkpoint nhỏ hỏi dùng bản nào: bản cloud (save trên máy được giữ lại để hoàn tác), bản trên máy (backup đè lên cloud sau khi thoát game), hoặc không mở game. Trong Cài đặt có thể đổi thành luôn mở game hoặc không bao giờ mở. Khi mất mạng, game vẫn luôn chạy. Một thông báo hệ thống cho bạn biết kết quả (đã restore, đã backup, bị lỗi, hoặc cần bạn quyết định); tắt được trong Cài đặt. Mọi bước đều ghi vào log.
- Điều khiển bằng tay cầm: D-pad hoặc cần trái đi từ game này sang game kế theo đúng thứ tự (qua phải ở cuối hàng thì sang hàng sau), **A** mở menu của game (Backup, Restore, Save cũ, Đánh dấu, Chi tiết) hoặc bấm nút, **X** đánh dấu game, **B** đóng hộp thoại hoặc quay về thanh bên, **LB / RB** chuyển màn hình. Khi dùng tay cầm, thanh bên hiện chú thích các nút. Phím mũi tên, Enter, Space và Esc cũng làm được y như vậy, cho layout Steam Input gửi phím.
- Cài rclone ngay trong app: nhận biết hệ điều hành, tải rclone chính thức có kiểm tra SHA-256, kết nối Google Drive bằng OAuth client ID của bạn. Sau khi kết nối, Cài đặt hiện client ID và secret đang lưu, ẩn cho tới khi bạn bấm nút con mắt, để copy sang các máy khác.
- Phiên bản app hiện ngay cạnh logo.
- Cập nhật ngay trong app: khi mở, app hỏi GitHub bản phát hành mới nhất và cho xem có gì mới. Bấm **Cập nhật và khởi động lại** thì app tải bản mới, kiểm tra chữ ký rồi cài (chạy bộ cài trên Windows, hoặc thay luôn file AppImage). Không cài gì cho tới khi bạn bấm. Tắt kiểm tra ở **Cài đặt → Cập nhật**. Bản Linux không chạy từ file AppImage thì chỉ có link tới trang tải.

## Ngôn ngữ

App có tiếng Anh và tiếng Việt, mặc định theo ngôn ngữ hệ thống, đổi trong **Cài đặt → Ngôn ngữ**. Bản dịch nằm ở [`src-tauri/locales/`](src-tauri/locales) (`en.yml`, `vi.yml`), dùng chung cho backend Rust ([rust-i18n](https://crates.io/crates/rust-i18n)) và giao diện. Muốn thêm ngôn ngữ: copy `en.yml`, dịch, rồi thêm vào `LANGUAGES` trong `config.rs` và `LOCALE_FILES` trong `main.rs`; unit test sẽ kiểm tra đủ khóa. Log luôn ghi bằng tiếng Anh.

Trang web (`site/`) dùng [i18next](https://www.i18next.com/) với `site/locales/<lang>.json`.

## Báo lỗi

- App ghi log vào `%LOCALAPPDATA%\Checkpoint\logs\checkpoint.log` (Linux: `~/.local/share/Checkpoint/logs/checkpoint.log`). Quá 2 MB thì chuyển thành `checkpoint.1.log`; giữ 3 file cũ gần nhất. Đường dẫn thư mục người dùng được thay bằng `~`; không bao giờ ghi token, client secret hay tên tài khoản Steam.
- Khi có lỗi, app hiện popup có nút **Tạo issue trên GitHub**. Nút này mở trang tạo issue trong trình duyệt với phiên bản, hệ điều hành, thông báo lỗi và phần cuối của log điền sẵn (SteamID được che). App không chứa thông tin đăng nhập GitHub nào: bạn đăng nhập GitHub trên trình duyệt rồi bấm Submit.
- Nút **Báo lỗi / Góp ý** ở thanh bên mở form báo lỗi hoặc đề xuất tính năng, và có nút copy log hoặc mở thư mục log.
- **Báo lỗi game này** trong hộp thoại **⋯** của mỗi game điền sẵn tên game, trạng thái, thư mục save, và chỉ những dòng log của game đó cộng các cảnh báo, lỗi, để phần quan trọng vừa trong issue. Cần cả file log thì bấm **Mở thư mục log** rồi kéo file vào issue.

## Cách hoạt động

- Đọc `libraryfolders.vdf` và `appmanifest_*.acf` để biết game đã cài, `loginusers.vdf` để biết tài khoản.
- Mỗi file được lưu theo đường dẫn logic `win/winAppData/...`, `any/base/...`, `linux/xdgData/...`. Trên Linux, đường dẫn `win/*` được ghi vào `steamapps/compatdata/<appid>/pfx/drive_c/users/steamuser/...`.
- Trạng thái tính từ SHA-256 nội dung save và hash ghi lại lúc đồng bộ lần cuối trên máy này (`state.json`).
- Với Google Drive, app giữ một tiến trình `rclone rcd` chạy trên 127.0.0.1 (mật khẩu ngẫu nhiên, tắt khi đóng app) thay vì chạy rclone cho mỗi file, nên kết nối Drive luôn sẵn. Nếu không khởi động được thì app quay về chạy rclone từng lần như cũ.
- Steam đánh dấu chủ của mỗi thư mục save bằng file `steam_autocloud.vdf` và dọn thư mục thuộc tài khoản khác khi mở game. Khi restore, thư mục dùng chung (thư mục cài game, AppData, Documents) được giao cho tài khoản đang đăng nhập trên máy này; thư mục mang tên tài khoản thì giữ nguyên chủ.
- Trước mỗi lần restore, save hiện tại được nén vào `%LOCALAPPDATA%\Checkpoint\snapshots\<appid>` (Linux: `~/.local/share/Checkpoint/snapshots/<appid>`) kèm file `checkpoint-snapshot.json` mô tả bản đó, giữ 10 bản mỗi game. Quay lại một bản sẽ trả file về đúng như cũ (file tạo sau thời điểm đó bị xóa) và không đổi bản ghi lần đồng bộ cuối, nên nếu game có backup trên cloud thì sau đó sẽ hiện "local mới hơn".

Trên cloud:

```
Checkpoint/
├── index.json                          # mọi phiên bản của mọi game, mới nhất trước
└── games/<appid>/<UTC>_<device>.zip    # file save + checkpoint-meta.json
```

## Nơi lưu

- **Google Drive qua rclone (khuyên dùng, Windows và Linux):** vào Cài đặt, bấm **Cài rclone** nếu chưa có, dán **Client ID** và **Client secret** của Google, bấm **Kết nối Google Drive**. App chạy `rclone config create gdrive drive scope=drive.file client_id=… client_secret=…`, mở trình duyệt để đăng nhập Google, rồi dùng `gdrive:Checkpoint`. Nếu trình duyệt không tự mở (ví dụ Firefox bản Flatpak trên Bazzite), hộp thoại đăng nhập hiện link kèm nút **Copy link** và **Mở trong trình duyệt**: dán link vào trình duyệt trên cùng máy đó. Làm vậy trên mọi máy, cùng client ID và cùng tài khoản Google.
- **Thư mục:** thư mục bất kỳ, ví dụ `G:\My Drive\Checkpoint` nếu dùng Google Drive for Desktop trên Windows.

### Vì sao cần client ID riêng

Client ID dùng chung của rclone bị giới hạn rất nặng và đang bị Google ngừng. Với client ID riêng, mỗi thao tác mất khoảng 3 giây thay vì hơn 30 giây. Tạo miễn phí, mất 5–10 phút. Hướng dẫn từng bước có trên trang web (`/guide/google-drive`, file `site/guide/google-drive.html`). Tóm tắt:

1. Tạo project trong Google Cloud Console, bật Google Drive API.
2. Google Auth Platform: chọn External, rồi bấm **Publish app** để trạng thái là **In production**. Để ở Testing thì đăng nhập hết hạn sau 7 ngày.
3. Tạo OAuth client loại **Desktop app**, copy client ID và secret.

Quyền `drive.file` chỉ cho app thấy file do chính nó tạo, và không phải quyền nhạy cảm nên không cần Google xác minh.

## Build

Cần Rust (MSVC trên Windows). Frontend là HTML/JS thuần trong `ui/`, không cần Node khi phát triển.

```bash
cd src-tauri
cargo run              # chạy dev
cargo test             # unit test
cargo test smoke -- --ignored --nocapture   # quét Steam thật trên máy, backup thử vào thư mục tạm
```

Đóng gói (`.exe`, AppImage) cần Tauri CLI: `cargo install tauri-cli --version "^2" --locked`, rồi `cargo tauri build`. Trên Linux cần cài gói phát triển `webkit2gtk4.1` trước (trên Bazzite thì làm trong distrobox hoặc toolbox).

## Phát hành

Đẩy tag dạng `vX.Y.Z` là [.github/workflows/release.yml](.github/workflows/release.yml) chạy: unit test, build bộ cài Windows (NSIS) và AppImage cho Linux, rồi tạo GitHub Release. Số phiên bản lấy từ tag.

```bash
git tag v0.2.0
git push origin v0.2.0
```

Tag có dấu `-` (ví dụ `v0.2.0-beta.1`) được đánh dấu pre-release. Trình cập nhật trong app bỏ qua pre-release.

Bộ cài được ký cho trình cập nhật trong app, và workflow đăng thêm `latest.json`, app đọc file này ở `releases/latest/download/latest.json`. Cài đặt một lần:

1. `cargo install tauri-cli --version "^2" --locked`, rồi `cargo tauri signer generate -w ~/.tauri/checkpoint.key` (đặt mật khẩu).
2. Dán nội dung `~/.tauri/checkpoint.key.pub` vào `plugins.updater.pubkey` trong `src-tauri/tauri.conf.json` (test `updater_pubkey_is_set` báo lỗi cho tới khi làm bước này).
3. Thêm secret của repo: `TAURI_SIGNING_PRIVATE_KEY` (nội dung `~/.tauri/checkpoint.key`) và `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Thiếu khóa thì workflow dừng.

Nhớ backup khóa private. Mất khóa thì các bản đã cài không kiểm tra được bản mới, phải cập nhật tay một lần với khóa mới.

## Trang web

`site/` là trang tĩnh: trang chủ (`index.html`), các trang hướng dẫn (`guide/index.html` → `/guide`, `guide/google-drive.html`, `guide/switch-account.html`), cùng Privacy Policy và Terms of Service dùng cho màn hình xin quyền Google. Mọi trang dịch bằng i18next (tiếng Anh và tiếng Việt, chọn theo `?lng=`, lựa chọn lần trước, hoặc ngôn ngữ trình duyệt; `/en` và `/vi` chuyển hướng về trang chủ). `main.js` chèn header và footer dùng chung, và chuyển các anchor cũ `#guide`, `#google` của trang chủ sang trang hướng dẫn. Ảnh chụp app nằm trong `site/shots/` (mỗi ngôn ngữ một bản). Deploy lên Vercel với Root Directory là `site`. Số sao, lượt tải và nút tải lấy trực tiếp từ GitHub API.

## Ủng hộ

Checkpoint miễn phí và mã nguồn mở. Nếu app giữ được tiến trình chơi của bạn, bạn có thể [mời mình một ly cà phê trên Ko-fi](https://ko-fi.com/fr0stf0x) ☕. Báo lỗi và góp ý trên [GitHub Issues](https://github.com/fr0stf0x/checkpoint/issues) cũng giúp rất nhiều.

## Hạn chế

- Save gắn với SteamID có thể chỉ chơi được trên đúng account đó. Chuyển sang tài khoản khác được với nhiều game nhưng không phải tất cả (save mã hóa có kiểm tra tài khoản bên trong thì không sửa được).
- Trên Linux, chạy game 1 lần qua Proton trước khi restore bản từ Windows, để Proton tạo prefix.
- Chưa hỗ trợ save lưu trong Registry của Windows.
- Chưa tự khai báo được game không có trong manifest.

## Sắp làm

Xem [TODO.md](TODO.md). Nổi bật: launch wrapper (`checkpoint run -- %command%`), khóa "đang chơi ở máy X", tự khai báo game, ảnh game và ảnh đại diện tài khoản, làm lại trang web.

## Giấy phép

[MIT](LICENSE) © fr0stf0x. Vị trí save lấy từ [ludusavi-manifest](https://github.com/mtkennerly/ludusavi-manifest) (dữ liệu PCGamingWiki); truy cập Google Drive qua [rclone](https://rclone.org/), được tải về riêng.
