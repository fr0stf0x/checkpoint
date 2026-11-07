# Checkpoint – ghi chú dự án cho Claude

Đọc file này trước khi làm việc. README.md mô tả app làm gì và build thế nào; file này ghi **vì sao**, **yêu cầu gốc**, và **còn gì chưa làm**.

## Bối cảnh

Chủ dự án có 3 PC chơi Steam: 2 máy Windows 11, 1 máy Bazzite (Fedora Atomic, immutable). Steam chỉ chạy được offline mode nên không dùng được Steam Cloud. App này thay Steam Cloud: backup/restore save game qua Google Drive của chính chủ dự án (gói Google AI Pro, dung lượng dư).

## Yêu cầu gốc (từ chủ dự án)

1. Chạy được trên Windows và Bazzite.
2. Liệt kê game đang có trong máy.
3. Liệt kê các account Steam.
4. Kết nối tới 1 thư mục tổng trên Google Drive.
5. Mỗi game có nút backup và restore.
6. Có nút backup và restore toàn bộ.
7. Khi restore, so ngày bản cloud với bản local; local mới hơn thì cảnh báo.

Sau đó bổ sung:
- App tự nhận biết hệ điều hành, tự kiểm tra có rclone chưa, có nút cài rclone, có ô nhập Google client ID + secret để tự chạy lệnh `rclone config`.
- GitHub Actions tự build khi đánh tag.
- Trang web (`site/`, deploy Vercel) có thông tin GitHub, số lượt tải, số sao, hướng dẫn sử dụng (đặc biệt phần kết nối Google), có bản tiếng Việt và tiếng Anh.

Tất cả các mục trên đã làm xong (v0.1.0).

## Kết luận khả thi (đã research)

- **Không làm được:** dùng chính Steam Cloud khi offline; đảm bảo restore save sang account Steam khác (nhiều game nhúng SteamID); tự tìm save cho 100% game; sync khi game đang chạy; gộp 2 save khác nhau.
- **Hạn chế:** save trong Registry (Windows ↔ Proton `user.reg`) chưa hỗ trợ; game bản Linux native khác đường dẫn và có thể khác định dạng save so với bản Windows, nên trên Bazzite ưu tiên chạy qua Proton; giao diện trong Gaming Mode của Bazzite khó dùng bằng tay cầm.
- **Quyết định thiết kế:** không so bằng ngày sửa file (lệch đồng hồ, giải nén làm đổi ngày) mà so bằng hash nội dung + hash lúc đồng bộ lần cuối (`state.json`), nhờ vậy phân biệt được local mới hơn / cloud mới hơn / xung đột. Cảnh báo cả hai chiều: restore khi local mới hơn, và backup khi cloud mới hơn.
- **Google Drive:** dùng rclone làm lớp vận chuyển (Drive for Desktop không có bản Linux). Client ID dùng chung của rclone bị Google giới hạn nặng (~37 giây mỗi lệnh) và đang bị khai tử → bắt buộc client ID riêng, scope `drive.file`, OAuth app phải **Publish (In production)**, nếu để Testing thì token hết hạn sau 7 ngày. Với `drive.file`, client ID khác nhau không thấy file của nhau → mọi máy phải dùng cùng client ID và cùng tài khoản Google.
- Sau khi đăng nhập Google, trình duyệt báo "127.0.0.1 refused to connect" là **bình thường** (rclone đã nhận mã và tắt máy chủ tạm), không phải bug.

## Prototype

Bản thiết kế tương tác ban đầu (mockup + bảng khả thi + đề xuất + kiến trúc): https://claude.ai/artifact/VQfARuNDZkth8BvZt6oVLy
Giao diện app thật (`ui/`) đi theo prototype này: sidebar Game / Tài khoản / Lịch sử / Cài đặt, bảng game với pill trạng thái, hộp thoại Restore so 2 bên Local vs Cloud, màn hình "kế hoạch" trước khi chạy Backup/Restore toàn bộ.

## Đề xuất và trạng thái

| Đề xuất | Trạng thái |
|---|---|
| So sánh bằng hash, phân biệt 3 trạng thái + xung đột | Xong |
| Cảnh báo cả chiều backup (cloud mới hơn) | Xong |
| Lịch sử phiên bản (giữ 10 bản/game), restore bản cũ | Xong |
| Snapshot local trước mỗi lần restore (giữ 5) | Xong (chưa có nút restore snapshot trong UI) |
| Chặn thao tác khi game đang chạy | Xong |
| Đánh dấu game có save gắn SteamID | Xong |
| Tách "save" và "cài đặt đồ họa" (mặc định không backup config) | Xong |
| Chỉ upload khi có thay đổi (hash giống thì bỏ qua) | Xong |
| Map đường dẫn Windows ↔ Proton prefix | Xong (chưa chạy thử trên Bazzite) |
| **Launch wrapper** `checkpoint run -- %command%` đặt trong Launch Options của Steam: tự restore trước khi chơi, tự backup sau khi tắt (đề xuất đáng làm nhất) | Chưa làm |
| **Khóa "đang chơi ở máy X"** (`lock.json` trên Drive) để tránh xung đột | Chưa làm |
| Game ngoài Steam / giả lập / tự khai báo đường dẫn (custom entry) | Chưa làm |
| Backup định kỳ chạy ngầm + icon khay hệ thống | Chưa làm |
| **Chuyển save sang tài khoản Steam khác** (A → B), xem mục bên dưới | Xong (`migrate.rs`), chưa thử với game thật |
| Save trong Registry | Chưa làm |
| Giữ 1 tiến trình `rclone rcd` chạy nền thay vì gọi rclone mỗi lệnh (~3 giây/lệnh hiện tại) | Chưa làm |
| Giao diện tiếng Anh cho app (hiện app chỉ có tiếng Việt, site có cả 2) | Chưa làm |

## Use case: chơi tiếp bằng tài khoản khác (A → B)

Yêu cầu của chủ dự án: chơi game (ví dụ Phantom Blade Zero) bằng tài khoản A, hôm sau không dùng A được nữa, muốn chơi tiếp bằng tài khoản B.

Phân loại save (quyết định app làm được gì):
- Không gắn account → không cần làm gì.
- SteamID/AccountID chỉ nằm trong tên thư mục (hoặc `userdata/<accountid>`) → copy sang thư mục của B, sửa `steam_autocloud.vdf` (`"accountid"`) sang B. Làm được.
- ID nằm trong file ở dạng đọc được (chữ hoặc u64 little-endian) → thay ID cùng độ dài, đánh dấu thử nghiệm (có thể vỡ checksum).
- File mã hóa/nén → không biết game có kiểm tra ID không, phải copy rồi thử trong game, có hoàn tác.
- Save ở server → không làm được.

Phantom Blade Zero (appid 4115450): save ở `<base>/ProjectPBZ/Saved/SaveGames/Steam/<SteamID64>/` gồm `PBZSaveGame.sav`, thư mục `1RealTime`, `2Hourly`, `3Daily`, và `steam_autocloud.vdf`. File `.sav` có header `PBZS`, entropy ~8 bit/byte (mã hóa hoặc nén), không có SteamID ở dạng đọc được → thuộc nhóm "phải thử".

UI đã chốt trong mockup: trình hướng dẫn 3 bước (Chọn A → B và game → Chuẩn bị: checklist + bảng đổi đường dẫn → Thử trong game: "Chạy được, giữ" / "Không được, hoàn tác"). Lối vào: nút trên thẻ tài khoản, menu "⋯" của game có save gắn SteamID, và hộp thoại Restore tự phát hiện bản cloud của account khác. Sau khi chuyển thành công, lưu cặp A → B lên Drive để các máy khác restore tự ghi vào B. Luôn copy, không bao giờ sửa hay xóa bản của A; chụp snapshot save hiện có của B trước khi ghi.

Đã cài đặt (`src-tauri/src/migrate.rs`):
- `migrate_preview` phân loại none/folder/in_file/opaque, kiểm tra B đã có thư mục save chưa, số file sẽ ghi đè, game có đang chạy.
- `migrate_apply` ghi bản ghi `<data>/migrations/<id>.json` **trước** khi copy (lỗi giữa chừng vẫn hoàn tác được), chụp file sẽ bị ghi đè vào `<id>.zip`, nhớ thư mục mới tạo. Trạng thái `pending` tới khi người dùng chọn.
- `migrate_undo` xóa thư mục mới tạo (kể cả file game tự tạo khi chơi thử), trả file bị ghi đè. `migrate_keep` ghi `account_aliases[A] = B` vào `index.json` trên cloud.
- Restore có `remap_from`/`remap_to`: ghi bản của A vào chỗ của B, sửa `steam_autocloud.vdf`. Hộp thoại Restore mặc định chọn theo `account_aliases`.
- Mỗi game chỉ có 1 lần chuyển đang chờ. Banner ở màn hình Game nhắc tới khi chọn Giữ/Hoàn tác.

## Quy ước làm việc

- Trao đổi với chủ dự án bằng tiếng Việt, xưng "tao/mày" theo cách chủ dự án dùng. Giao diện app và comment code viết tiếng Việt; README.md và `site/en.html` có bản tiếng Anh, giữ 2 bản đồng bộ khi sửa.
- Frontend là HTML/JS thuần trong `ui/` (không Node, không bundler). Sau khi sửa `ui/` phải build lại Rust vì Tauri nhúng frontend vào binary.
- Trên máy Windows của chủ dự án, PowerShell mở sẵn có thể chưa có PATH mới: thêm `$env:Path = [Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [Environment]::GetEnvironmentVariable("Path","User") + ";$env:USERPROFILE\.cargo\bin"` trước khi gọi `cargo`/`git`.
- Phải tắt `checkpoint.exe` đang chạy trước khi `cargo build`, vì file exe bị khóa.
- Test thật trên máy: `cargo test smoke -- --ignored --nocapture` (quét Steam, backup vào thư mục tạm), `cargo test drive_real -- --ignored --nocapture` (backup thật lên nơi lưu đang cấu hình), `cargo test install_real -- --ignored --nocapture` (tải rclone thật). Không restore đè lên save thật khi test.
- Phát hành: đánh tag `vX.Y.Z` và push tag. Hỏi chủ dự án trước khi push hoặc đánh tag.
- Không bao giờ in hay commit token rclone, client secret, hay nội dung `rclone.conf`.
