# Checkpoint

Backup / restore save game Steam giữa nhiều máy (Windows + Bazzite) qua Google Drive, dùng được khi Steam chạy offline mode.

## Cách hoạt động

- Đọc `libraryfolders.vdf` + `appmanifest_*.acf` để biết game đã cài, `loginusers.vdf` để biết tài khoản.
- Vị trí save lấy từ [ludusavi-manifest](https://github.com/mtkennerly/ludusavi-manifest) (PCGamingWiki), cộng thêm thư mục `userdata/<account>/<appid>/remote` của Steam Cloud.
- Mỗi file được lưu theo đường dẫn logic `win/winAppData/...`, `any/base/...`, `linux/xdgData/...`. Khi restore trên Bazzite, đường dẫn `win/*` được ghi vào Proton prefix `steamapps/compatdata/<appid>/pfx/drive_c/users/steamuser/...`.
- Trạng thái so bằng hash SHA-256 nội dung save và hash lúc đồng bộ lần cuối (`state.json`), nên phân biệt được: local mới hơn, cloud mới hơn, xung đột.
- Trước mỗi lần restore, bản local được chụp snapshot vào `%LOCALAPPDATA%\Checkpoint\snapshots` (giữ 5 bản).

Trên cloud:

```
Checkpoint/
├── index.json                          # mọi phiên bản của mọi game, mới nhất trước
└── games/<appid>/<UTC>_<device>.zip    # file save + checkpoint-meta.json
```

## Nơi lưu

- **rclone (khuyên dùng, chạy cả 2 OS):** cài rclone, vào Cài đặt → "Kết nối Google Drive". App chạy `rclone config create gdrive drive scope=drive.file` (mở trình duyệt đăng nhập Google), remote mặc định `gdrive:Checkpoint`. Làm vậy trên cả 3 máy với cùng tài khoản Google.
- **Thư mục:** ví dụ `G:\My Drive\Checkpoint` nếu dùng Google Drive for Desktop trên Windows.

## Build

Cần Rust (MSVC trên Windows). Frontend là HTML/JS thuần trong `ui/`, không cần Node.

```bash
cd src-tauri
cargo run              # chạy dev
cargo test             # unit test
cargo test smoke -- --ignored --nocapture   # quét Steam thật trên máy, backup thử vào thư mục tạm
```

Đóng gói (`.exe`/`.msi`, AppImage cho Bazzite) cần `cargo install tauri-cli --version "^2" --locked` rồi `cargo tauri build`.
Trên Bazzite build trong distrobox/toolbox có `webkit2gtk4.1-devel`, hoặc build AppImage trên máy Linux khác.

## Chưa làm

- Launch wrapper `checkpoint run -- %command%` (tự restore trước khi chơi, backup sau khi tắt)
- Khóa "đang chơi ở máy X"
- Save trong Registry
- Restore snapshot local từ giao diện (hiện mở thư mục snapshots bằng tay)
