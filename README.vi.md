# Checkpoint

[English](README.md) · **Tiếng Việt**

Backup và restore save game Steam giữa các máy của bạn (Windows và Linux, kể cả Bazzite) qua chính Google Drive của bạn. Dùng được khi Steam chạy offline mode, lúc Steam Cloud không hoạt động.

Tải về: [bản mới nhất](https://github.com/fr0stf0x/checkpoint/releases/latest) · Trang web và hướng dẫn: xem thư mục `site/`

## Tính năng

- Liệt kê game Steam đã cài và các tài khoản Steam trên máy.
- Tìm file save bằng [ludusavi-manifest](https://github.com/mtkennerly/ludusavi-manifest) (dữ liệu PCGamingWiki, khoảng 20.000 game), cộng thêm thư mục `userdata/<account>/<appid>/remote` của Steam Cloud.
- Backup và restore từng game, hoặc toàn bộ với màn hình xem trước game nào chạy, game nào bỏ qua.
- So sánh bằng hash nội dung, không dựa vào ngày sửa file. Phân biệt "local mới hơn", "cloud mới hơn" và xung đột thật, cảnh báo trước khi nút nào đó ghi đè tiến trình mới hơn.
- Giữ 10 bản gần nhất mỗi game trên cloud. Chụp snapshot bản local trước mỗi lần restore.
- Không cho backup hoặc restore khi game đang chạy. Đánh dấu game có save gắn với SteamID.
- Save tạo trên Windows restore được vào Proton prefix trên Linux và ngược lại.
- Chuyển save của một game từ tài khoản Steam này sang tài khoản khác (ví dụ khi phải đổi tài khoản): copy thư mục save sang tài khoản mới, sửa `steam_autocloud.vdf`, tùy chọn thay SteamID bên trong file save, rồi để bạn thử trong game và **Giữ** hoặc **Hoàn tác** bằng 1 click. File của tài khoản cũ không bao giờ bị đụng tới. Restore cũng ghi được bản backup vào tài khoản khác.
- Cài rclone ngay trong app: nhận biết hệ điều hành, tải rclone chính thức có kiểm tra SHA-256, kết nối Google Drive bằng OAuth client ID của bạn.

## Báo lỗi

- App ghi log vào `%LOCALAPPDATA%\Checkpoint\logs\checkpoint.log` (Linux: `~/.local/share/Checkpoint/logs/checkpoint.log`), quá 2 MB thì xoay vòng. Đường dẫn thư mục người dùng được thay bằng `~`; không bao giờ ghi token, client secret hay tên tài khoản Steam.
- Khi có lỗi, app hiện popup có nút **Tạo issue trên GitHub**. Nút này mở trang tạo issue trong trình duyệt với phiên bản, hệ điều hành, thông báo lỗi và phần cuối của log điền sẵn (SteamID được che). App không chứa thông tin đăng nhập GitHub nào: bạn đăng nhập GitHub trên trình duyệt rồi bấm Submit.
- Nút **Báo lỗi / Góp ý** ở thanh bên mở form báo lỗi hoặc đề xuất tính năng, và có nút copy log hoặc mở thư mục log.

## Cách hoạt động

- Đọc `libraryfolders.vdf` và `appmanifest_*.acf` để biết game đã cài, `loginusers.vdf` để biết tài khoản.
- Mỗi file được lưu theo đường dẫn logic `win/winAppData/...`, `any/base/...`, `linux/xdgData/...`. Trên Linux, đường dẫn `win/*` được ghi vào `steamapps/compatdata/<appid>/pfx/drive_c/users/steamuser/...`.
- Trạng thái tính từ SHA-256 nội dung save và hash ghi lại lúc đồng bộ lần cuối trên máy này (`state.json`).
- Trước mỗi lần restore, save hiện tại được nén vào `%LOCALAPPDATA%\Checkpoint\snapshots` (Linux: `~/.local/share/Checkpoint/snapshots`), giữ 5 bản.

Trên cloud:

```
Checkpoint/
├── index.json                          # mọi phiên bản của mọi game, mới nhất trước
└── games/<appid>/<UTC>_<device>.zip    # file save + checkpoint-meta.json
```

## Nơi lưu

- **Google Drive qua rclone (khuyên dùng, Windows và Linux):** vào Cài đặt, bấm **Cài rclone** nếu chưa có, dán **Client ID** và **Client secret** của Google, bấm **Kết nối Google Drive**. App chạy `rclone config create gdrive drive scope=drive.file client_id=… client_secret=…`, mở trình duyệt để đăng nhập Google, rồi dùng `gdrive:Checkpoint`. Làm vậy trên mọi máy, cùng client ID và cùng tài khoản Google.
- **Thư mục:** thư mục bất kỳ, ví dụ `G:\My Drive\Checkpoint` nếu dùng Google Drive for Desktop trên Windows.

### Vì sao cần client ID riêng

Client ID dùng chung của rclone bị giới hạn rất nặng và đang bị Google ngừng. Với client ID riêng, mỗi thao tác mất khoảng 3 giây thay vì hơn 30 giây. Tạo miễn phí, mất 5–10 phút. Hướng dẫn từng bước có trên trang web (`site/index.html`, mục *Kết nối Google Drive*). Tóm tắt:

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

Tag có dấu `-` (ví dụ `v0.2.0-beta.1`) được đánh dấu pre-release.

## Trang web

`site/` là trang tĩnh (trang chủ, hướng dẫn, Privacy Policy, Terms of Service), tiếng Việt ở `/` và tiếng Anh ở `/en`. Deploy lên Vercel với Root Directory là `site`. Số sao, lượt tải và nút tải lấy trực tiếp từ GitHub API.

## Hạn chế

- Save gắn với SteamID có thể chỉ chơi được trên đúng account đó. Chuyển sang tài khoản khác được với nhiều game nhưng không phải tất cả (save mã hóa có kiểm tra tài khoản bên trong thì không sửa được).
- Trên Linux, chạy game 1 lần qua Proton trước khi restore bản từ Windows, để Proton tạo prefix.
- Chưa hỗ trợ save lưu trong Registry của Windows.
- Chưa tự khai báo được game không có trong manifest.

## Sắp làm

- Launch wrapper `checkpoint run -- %command%` (tự restore trước khi chơi, backup sau khi tắt)
- Khóa "đang chơi ở máy X"
- Tự khai báo game
- Restore snapshot local từ giao diện
