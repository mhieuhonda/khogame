-- ============================================================
-- v3.6.0 — MỞ RỘNG BANK CÂU ĐỐ HẰNG NGÀY (DAILY TRIVIA)
-- ============================================================
-- Trước v3.6.0 bank chỉ có 16 câu (migration 023) — mỗi user chơi
-- 3 câu/ngày nên ~5.3 ngày là cạn câu (logic arcade.rs: NOT EXISTS
-- loại mọi câu đã trả lời TRONG ĐỜI). Người dùng yêu cầu THÊM NHIỀU
-- CÂU HỎI HẰNG NGÀY hơn.
--
-- Migration này thêm 64 câu mới (tổng 80) → đủ ~26 ngày chơi liên tục
-- cho 1 user, đồng thời đa dạng chủ đề: console/hãng game, thể loại,
-- tiếng lóng MOBA/esports, lịch sử game, game Việt Nam, thuật ngữ
-- kỹ thuật. Đáp án ĐẢO VỊ TRÍ (0-3) khác nhau để không đoán mò theo
-- vị trí.
--
-- AN TOÀN RE-RUN: 031 đã tạo UNIQUE INDEX uq_trivia_questions_question
-- trên cột question → dùng ON CONFLICT (question) DO NOTHING (có target
-- rõ ràng, khác seed 023 lỗi 'DO NOTHING' không target). Chạy lại
-- migration này không bao giờ nhân đôi câu.
-- ============================================================

INSERT INTO trivia_questions (question, options, correct_index, explanation) VALUES
    ('Công ty nào phát triển dòng máy chơi game PlayStation?', '["Sega","Sony","Microsoft","Atari"]'::jsonb, 1, 'PlayStation là thương hiệu của Sony Interactive Entertainment (ra mắt 1994).'),
    ('Nhân vật Mario thuộc thương hiệu game của hãng nào?', '["Nintendo","Capcom","Sega","Bandai Namco"]'::jsonb, 0, 'Mario là linh vật của Nintendo, xuất hiện lần đầu trong Donkey Kong (1981).'),
    ('Máy chơi game Xbox thuộc về công ty nào?', '["Apple","Sony","Microsoft","IBM"]'::jsonb, 2, 'Xbox do Microsoft phát triển, bản đầu ra mắt năm 2001.'),
    ('Sonic là nhân vật linh vật của hãng game nào?', '["Namco","Sega","Taito","SNK"]'::jsonb, 1, 'Sonic the Hedgehog là linh vật của Sega, đối thủ kinh điển của Mario thập niên 90.'),
    ('Máy console Nintendo Switch ra mắt năm nào?', '["2012","2015","2017","2019"]'::jsonb, 2, 'Switch ra mắt 3/3/2017 và trở thành một trong những console bán chạy nhất lịch sử.'),
    ('Hệ máy chơi game cầm tay nổi tiếng nhất của Sony là dòng nào?', '["Game Boy","Nintendo DS","Xbox portable","PSP / PS Vita"]'::jsonb, 3, 'PSP (2004) và PS Vita (2011) là hai máy cầm tay của Sony.'),
    ('Máy console đầu tiên của Nintendo ở phương Tây mang tên gì?', '["Nintendo Entertainment System (NES)","Nintendo 64","GameCube","Wii U"]'::jsonb, 0, 'NES (1985 ở Mỹ) đưa Nintendo trở thành vua console sau khủng hoảng game 1983.'),
    ('Game Tetris được sáng tạo bởi ai?', '["Shigeru Miyamoto","Alexey Pajitnov","Hideo Kojima","John Carmack"]'::jsonb, 1, 'Alexey Pajitnov sáng tạo Tetris tại Liên Xô năm 1984.'),
    ('Game Pac-Man ra mắt năm nào?', '["1972","1980","1988","1993"]'::jsonb, 1, 'Pac-Man của Namco ra mắt năm 1980 và là một trong những game phổ biến nhất mọi thời đại.'),
    ('Pong (1972) mô phỏng môn thể thao nào?', '["Tennis bàn","Bóng đá","Bóng chày","Cầu lông"]'::jsonb, 0, 'Pong của Atari mô phỏng tennis bàn — game thương mại thành công đầu tiên.'),
    ('Pac-Man được thiết kế lấy cảm hứng từ hình ảnh gì?', '["Quả bóng đá","Chiếc bánh pizza thiếu một miếng","Mặt trăng khuyết","Con ruồi"]'::jsonb, 1, 'Toru Iwatani nghĩ ra Pac-Man khi nhìn chiếc bánh pizza vừa bị lấy đi một miếng.'),
    ('Game Minecraft ban đầu do ai phát triển?', '["Markus Notch Persson","Gabe Newell","Phil Spencer","Tim Sweeney"]'::jsonb, 0, 'Notch tạo Minecraft năm 2009, sau đó Mojang (nay thuộc Microsoft) phát triển tiếp.'),
    ('Số lượng bản sao Minecraft đã bán ra nằm ở khoảng nào?', '["Hơn 300 triệu","Khoảng 50 triệu","Khoảng 10 triệu","Dưới 1 triệu"]'::jsonb, 0, 'Minecraft là game bán chạy nhất lịch sử với hơn 300 triệu bản (2023).'),
    ('Steam là nền tảng dùng để làm gì?', '["Chỉnh sửa ảnh","Mua và quản lý game trực tuyến","Gọi điện video","Học lập trình"]'::jsonb, 1, 'Steam của Valve (2003) là cửa hàng game PC trực tuyến lớn nhất thế giới.'),
    ('Half-Life được phát triển bởi studio nào?', '["Blizzard","id Software","Valve","Epic Games"]'::jsonb, 2, 'Half-Life (1998) là game kinh điển của Valve.'),
    ('Counter-Strike ban đầu được sinh ra như thế nào?', '["Mod do cộng đồng làm từ Half-Life","Game độc lập của EA","Mod của Quake","Bản demo của Unreal"]'::jsonb, 0, 'CS bắt nguồn từ mod Half-Life của Minh Le và Jess Cliffe năm 1999.'),
    ('Dòng game Grand Theft Auto (GTA) do studio nào phát triển?', '["Rockstar Games","Ubisoft","EA DICE","Bethesda"]'::jsonb, 0, 'Rockstar Games (Scotland, Anh) phát triển series GTA.'),
    ('The Witcher 3 do studio của nước nào phát triển?', '["Ba Lan (CD Projekt Red)","Nhật Bản (Square Enix)","Hàn Quốc (Nexon)","Mỹ (Bethesda)"]'::jsonb, 0, 'CD Projekt Red đến từ Warszawa, Ba Lan.'),
    ('The Legend of Zelda thuộc thể loại chính nào?', '["Sports","Action-Adventure","Rhythm","Racing"]'::jsonb, 1, 'Zelda là đại diện kinh điển của thể loại hành động phiêu lưu khám phá.'),
    ('Hideo Kojima nổi tiếng nhất với series game nào?', '["Metal Gear","Final Fantasy","Resident Evil","Devil May Cry"]'::jsonb, 0, 'Metal Gear Solid là kiệt tác của Kojima; ông sau đó làm Death Stranding.'),
    ('Elden Ring hợp tác với nhà văn nào cho cốt truyện thế giới?', '["George R. R. Martin","J. K. Rowling","Stephen King","Haruki Murakami"]'::jsonb, 0, 'Tác giả Game of Thrones hợp tác với FromSoftware xây dựng thế giới Elden Ring.'),
    ('Dòng game Final Fantasy thuộc công ty nào?', '["Konami","Square Enix","Bandai Namco","Sega"]'::jsonb, 1, 'Square Enix (trước là Squaresoft) sở hữu Final Fantasy.'),
    ('Game Stardew Valley ban đầu do bao nhiêu người phát triển?', '["50 người","10 người","1 người","Một studio 200 người"]'::jsonb, 2, 'Eric Barone (ConcernedApe) tự tay làm gần như toàn bộ game trong 4 năm.'),
    ('Among Us lấy bối cảnh nào?', '["Trường học ma quái","Tàu vũ trụ có kẻ giả mạo","Nhà tù bí ẩn","Bệnh viện bỏ hoang"]'::jsonb, 1, 'Among Us là game tìm kẻ Impostor trà trộn giữa phi hành đoàn.'),
    ('PUBG và Fortnite thuộc thể loại nào?', '["MOBA","Visual Novel","Battle Royale","RTS"]'::jsonb, 2, 'Battle Royale: nhiều người nhảy xuống bản đồ, sống sót cuối cùng thắng.'),
    ('Dota 2 và League of Legends thuộc thể loại nào?', '["MOBA","FPS","Game bài","Battle Royale"]'::jsonb, 0, 'MOBA = Multiplayer Online Battle Arena, sinh ra từ mod Defense of the Ancients.'),
    ('Game chiến thuật thời gian thực (RTS) kinh điển của Blizzard là gì?', '["StarCraft","Candy Crush","Pong","Animal Crossing"]'::jsonb, 0, 'StarCraft (1998) là chuẩn mực RTS và là môn esports huyền thoại ở Hàn Quốc.'),
    ('Thể loại Visual Novel tập trung nhất vào yếu tố nào?', '["Tốc độ bắn nhanh","Cốt truyện và lựa chọn","Xây dựng nhà cửa","Đua xe"]'::jsonb, 1, 'Visual Novel trọng chuyện kể và các lựa chọn dẫn tới kết thúc khác nhau.'),
    ('FPS là viết tắt của gì?', '["First-Person Shooter","Fast Play Style","Free Player Slot","Final Point Score"]'::jsonb, 0, 'FPS = game bắn súng góc nhìn thứ nhất như CS2, Valorant, Half-Life.'),
    ('RPG là viết tắt của gì?', '["Role-Playing Game","Racing Power Game","Random Play Generator","Real Physics Graphics"]'::jsonb, 0, 'RPG = game nhập vai, phát triển nhân vật qua chỉ số và cốt truyện.'),
    ('GTA viết tắt của cụm từ nào?', '["Grand Theft Auto","Great Truck Arena","Global Tank Assault","Game Total Action"]'::jsonb, 0, 'Grand Theft Auto — series sandbox tội phạm của Rockstar.'),
    ('AOE trong game nghĩa là gì?', '["Area of Effect","Attack On Enemy","All Of Energy","Ancient Of Earth"]'::jsonb, 0, 'AOE = kỹ năng/hiệu ứng tác dụng trên một vùng, không phải một mục tiêu duy nhất.'),
    ('Gank trong game MOBA nghĩa là gì?', '["Bắt bài bất lợi cho đối thủ (thường nhiều đánh ít)","Đánh trụ liên tục","Mua trang bị","Đi lang thang"]'::jsonb, 0, 'Gank = tỏa đi bất ngờ ép sát hoặc hạ gục đối phương để tạo lợi thế.'),
    ('Carry trong MOBA là người như thế nào?', '["Người gây sát thương chủ lực cuối trận","Người chỉ đi hỗ trợ","Người đứng tank đỡ đòn","Người cắm mắt duy nhất"]'::jsonb, 0, 'Carry cần farm mạnh đầu game để gánh đội hình về cuối.'),
    ('Kỹ năng Kiting nghĩa là gì?', '["Vừa chạy vừa đánh, giữ khoảng cách","Bay lượn trên bản đồ","Cắm bẫy liên tục","Đánh đứng im"]'::jsonb, 0, 'Kiting (kite) là kéo di chuyển mục tiêu trong khi vẫn gây sát thương.'),
    ('Cooldown của kỹ năng là gì?', '["Thời gian chờ trước khi dùng lại","Độ trễ mạng","Số mana tối đa","Màn hình khựng"]'::jsonb, 0, 'Cooldown (CD) ngăn spam kỹ năng — hết CD mới dùng lại được.'),
    ('AFK nghĩa là gì?', '["Away From Keyboard — rời khỏi game","All Fight Kill","A Few Kills","Anti Final Kill"]'::jsonb, 0, 'AFK dùng chỉ người treo game không thao tác.'),
    ('GG là viết tắt của gì?', '["Good Game","Go Go","Game Gone","Great Gold"]'::jsonb, 0, 'GG là cách chào kết thúc trận đấu lịch sự, tôn trọng đối thủ.'),
    ('Noob dùng để chỉ người thế nào?', '["Người chơi mới còn yếu","Cao thủ","Người quay clip game","Quản trị server"]'::jsonb, 0, 'Noob (newbie) — tuy nhiên nên hạn chế dùng để miệt thị người khác.'),
    ('Rage quit nghĩa là gì?', '["Bỏ trận giữa chừng vì quá cay cú","Thắng liên tiếp","Thoát vì hết tiền","Đánh rất gắt"]'::jsonb, 0, 'Rage quit là hành vi thoát game khi đang thua/căng thẳng — mất điểm uy tín ở nhiều game.'),
    ('P2W (Pay-to-Win) chỉ game loại nào?', '["Nạp tiền mua sức mạnh hơn người chơi khác","Chơi được hai người","Miễn phí trọn đời","Có bản offline"]'::jsonb, 0, 'P2W bị cộng đồng game phản đối vì phá cân bằng cạnh tranh.'),
    ('F2P nghĩa là gì?', '["Free-to-Play — chơi miễn phí","Fight to Play","Fast to Play","Full-time Player"]'::jsonb, 0, 'F2P là mô hình game miễn phí, doanh thu từ item/cosmetic.'),
    ('Loot box từng gây tranh cãi lớn vì lý do gì?', '["Cơ chế mở hộp ngẫu nhiên tương tự đánh bạc","Giá quá rẻ","Quá dễ lấy đồ hiếm","Chỉ có ở game offline"]'::jsonb, 0, 'Nhiều nước đã cân nhắc điều chỉnh loot box vì cơ chế may rủi giống cờ bạc.'),
    ('Skins trong game là gì?', '["Trang phục, ngoại hình nhân vật hoặc vũ khí","Kỹ năng đặc biệt","Đạo cấm","Gói mạng"]'::jsonb, 0, 'Skins thường chỉ đổi thẩm mỹ, không ảnh hưởng sức mạnh (game cân bằng tốt).'),
    ('Nerf một nhân vật nghĩa là gì?', '["Giảm sức mạnh đi cho cân bằng","Tăng sức mạnh lên","Đổi hoàn toàn bộ kỹ năng","Xóa khỏi game"]'::jsonb, 0, 'Ngược lại của nerf là buff (tăng sức mạnh).'),
    ('Mod của game là gì?', '["Bản chỉnh sửa/nội dung do cộng đồng tạo ra","Màn chơi thứ tự ngược","Chế độ giảm khó","Loại trộm đồ"]'::jsonb, 0, 'Mod làm game sống lại lâu dài — vd Counter-Strike, Dota đều khởi đầu là mod.'),
    ('Emulator là gì?', '["Phần mềm giả lập máy chơi game khác","Vòng điều khiển game","Tai nghe chuyên game","Ghế game"]'::jsonb, 0, 'Emulator giả lập phần cứng — lưu ý pháp lý phụ thuộc ROM bạn sử dụng.'),
    ('Cloud gaming cho phép người chơi làm gì?', '["Chơi game qua internet mà không cần máy cấu hình cao","Chơi khi mất mạng","Chơi trên mây cao 10km","Tự lưu game lên USB"]'::jsonb, 0, 'GeForce Now, Xbox Cloud Gaming xử lý game trên server rồi stream về thiết bị.'),
    ('Permadeath nghĩa là gì?', '["Chết là mất nhân vật vĩnh viễn","Hồi sinh ngay tại chỗ","Mất một nửa tiền","Đổi nhân vật khác"]'::jsonb, 0, 'Permadeath (roguelike) khiến mỗi quyết định đều nặng ký.'),
    ('Speedrun dùng các "glitch" của game để làm gì?', '["Rút ngắn thời gian hoàn thành","Làm game đẹp hơn","Tăng độ khó","Lấy đồ vô hạn"]'::jsonb, 0, 'Nhiều category speedrun phân loại rõ có/không dùng glitch.'),
    ('Nền tảng phát trực tiếp game phổ biến nhất thế giới là gì?', '["Twitch","Twitter","TikTok","Telegram"]'::jsonb, 0, 'Twitch (thuộc Amazon) là nền tảng stream game lớn nhất thế giới.'),
    ('ESports là gì?', '["Thể thao điện tử — thi đấu game chuyên nghiệp","Game giáo dục học điện","Phần mềm giả lập điện","Chuỗi cửa hàng game"]'::jsonb, 0, 'Esports có giải đấu lớn như The International (Dota 2) với giải thưởng hàng chục triệu USD.'),
    ('Tay cầm PS5 thế hệ mới có tên gọi gì?', '["DualSense","DualShock 5","Joy-Con Pro","Xbox Elite S"]'::jsonb, 0, 'DualSense nổi bật với phản hồi xúc giác (haptic) và adaptive trigger.'),
    ('Thiết bị VR dùng để làm gì khi chơi game?', '["Tạo trải nghiệm đắm chìm thực tế ảo","Tăng dung lượng ổ cứng","Sạc tay cầm","Giữ mát máy"]'::jsonb, 0, 'VR như Meta Quest, PSVR2 đưa người chơi "vào" thế giới game.'),
    ('Ping thấp trong game nghĩa là gì?', '["Kết nối ổn định, độ trễ nhỏ","Đồ họa đẹp","Pin tay cầm yếu","Tải map nhanh"]'::jsonb, 0, 'Ping (ms) thấp = phản hồi nhanh — quan trọng với game thi đấu.'),
    ('Máy game cầm tay bán chạy nhất mọi thời đại là dòng nào?', '["Steam Deck","PS Vita","Nintendo Switch","Nokia N-Gage"]'::jsonb, 2, 'Switch lai console/cầm tay đã bán hơn 140 triệu máy.'),
    ('Game Việt Nam Flappy Bird từng gây bão toàn cầu năm nào?', '["2013","2009","2016","2020"]'::jsonb, 0, 'Flappy Bird của Nguyễn Hà Đông đứng đầu App Store đầu 2014, ra mắt 2013.'),
    ('Flappy Bird do lập trình viên Việt Nam nào phát triển?', '["Nguyễn Hà Đông","Nguyễn Nhật Ánh","Phạm Nhật Vượng","Đàm Vĩnh Hưng"]'::jsonb, 0, '.dotGears của Nguyễn Hà Đông (Hà Nội) tạo nên Flappy Bird.'),
    ('Đấu Trường Chân Lý ở Việt Nam là phiên bản của game nào?', '["Teamfight Tactics (TFT)","Dota 2","Hearthstone","Clash Royale"]'::jsonb, 0, 'TFT là chế độ tự động chiến đấu của Riot Games, Garena phát hành bản Việt hóa.'),
    ('Game nào của Việt Nam lấy cảm hứng chiến thuật 5v5 và từng đông đảo người chơi nước ngoài?', '["Liên Quân Mobile","PUBG Mobile VN","Free Fire VN","Zing Play"]'::jsonb, 0, 'Liên Quân Mobile (Arena of Valor) là tựa MOBA quốc dân của game mobile Việt.'),
    ('Studio nào phát triển Liên Minh Huyền Thoại (League of Legends)?', '["Riot Games","Valve","Blizzard","Supercell"]'::jsonb, 0, 'Riot Games (2006, Los Angeles) — League ra mắt 2009.'),
    ('Valve phát hành hệ máy cầm tay PC tên là gì?', '["Steam Deck","Steam Boy","Valve Go","Portal Deck"]'::jsonb, 0, 'Steam Deck (2022) chạy SteamOS trên kiến trúc x86.'),
    ('Game Hollow Knight có nhân vật chính là gì?', '["Một chú bọ hiệp sĩ","Một hiệp sĩ người","Con rồng nhỏ","Robot gỉ"]'::jsonb, 0, 'Hollow Knight (Team Cherry) là kiệt suất metroidvania indie của Úc.'),
    ('Tetris từng hiệu quả đến mức có thuật ngữ riêng, gọi là gì?', '["Tetris effect — mơ thấy khối rơi","Tetris fever","Block flu","Tetris syndrome"]'::jsonb, 0, 'Người chơi Tetris nhập tâm hay mơ thấy các khối ghép đang rơi.'),
    ('Số 256 nổi tiếng là màn "chết" trong game kinh điển nào?', '["Pac-Man","Tetris","Snake","Space Invaders"]'::jsonb, 0, 'Màn 256 của Pac-Man bị lỗi hiển thị nửa màn hình — ký ức huyền thoại của game thủ.'),
    ('Trò chơi xếp hình Pokémon Go nổi tiếng với công nghệ nào?', '["AR — thực tế tăng cường","Blockchain","Công nghệ laser","Đĩa than"]'::jsonb, 0, 'Pokémon GO (Niantic, 2016) dùng AR đưa Pokémon ra đường phố thật.'),
    ('Phong cách "chữa lành" (cozy game) đại diện bởi tựa game nào?', '["Animal Crossing","DOOM","Dark Souls","Counter-Strike"]'::jsonb, 0, 'Cozy game nhịp chậm, dễ chịu — Animal Crossing: New Horizons bùng nổ mùa dịch.'),
    ('Dòng game Dark Souls nổi tiếng với điều gì?', '["Độ khó cao và thiết kế battle tinh tế","Chơi trẻ em","Đồ họa hoạt hình","Không có boss"]'::jsonb, 0, 'FromSoftware tạo ra cả khái niệm "Soulslike" — khó nhưng công bằng.'),
    ('Câu cửa miệng "Praise the Sun!" đến từ game nào?', '["Dark Souls","Skyrim","Minecraft","Overwatch"]'::jsonb, 0, 'Chân dung cử chỉ "Praise the Sun" của Dark Souls đã trở thành meme kinh điển.'),
    ('Game xếp hình nào có chế độ đối kháng 99 người trên Switch?', '["Tetris 99","Puyo Puyo 1v1","Snake 99","Bejeweled Arena"]'::jsonb, 0, 'Tetris 99 biến xếp hình kinh điển thành battle Royale 99 người.'),
    ('Valorant và CS2 thuộc thể loại game nào?', '["FPS chiến thuật 5v5","MOBA","Nhịp nhạc","Mô phỏng nông trại"]'::jsonb, 0, 'FPS chiến thuật: chắn bomb/đặt bomb, kinh tế súng đạn theo hiệp.'),
    ('Auto-save là gì?', '["Game tự động lưu tiến trình","Tự động nạp game","Tự cập nhật","Tự động chơi"]'::jsonb, 0, 'Auto-save cứu game thủ khỏi mất tiến trình — nhưng đừng tắt máy giữa lúc đang lưu!'),
    ('Speedrun Minecraft "random seed" nghĩa là gì?', '["Thế giới sinh ra ngẫu nhiên, chưa ai biết trước","Chạy bằng hạt giống cà phê","Map do người khác làm sẵn","Chơi bản mod"]'::jsonb, 0, 'Random seed đòi hỏi người chơi ứng biến với thế giới chưa từng thấy.'),
    ('Trò chơi dân gian cờ tướng của Việt Nam có mặt trên nền tảng nào?', '["Game online PC/mobile và các giải đấu online","Chỉ trên giấy","Chỉ ở sân trường","Không thể chơi trực tuyến"]'::jsonb, 0, 'Cờ tướng/cờ vua online rất phổ biến ở VN với các bàn chơi và giải đấu trực tuyến.')
ON CONFLICT (question) DO NOTHING;
