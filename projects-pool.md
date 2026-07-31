# Proje Havuzu — Beginner Grubu (Fiziksel Hafta)

Bu dosya, öğrencilere görev panosundan verilecek yeni projelerin **konsept havuzu**. Cem
skorlu task'ları bu konseptlere göre yazacak; burada her proje için senaryo tohumu, ana
mekanik, taslak learning goal'lar ve **doğrulanmış kaynak linkleri** var.

## Format hatırlatması (portal ile birebir)

Panodaki her görev şu alanlardan oluşuyor:

- **title** — kısa, akılda kalıcı başlık
- **level** — funding round'a göre zorluk:
  - `PRESEED` = Beginner (100 puan)
  - `SEED` = Intermediate (400 puan)
  - `SERIES_A` = Advanced (700 puan)
- **description** — düz metin (portal `white-space:pre-wrap` ile birebir gösteriyor;
  markdown render **edilmiyor**). Yapı:
  ```
  Görev tanımı: <senaryo>...

  - <alt görev> (+X puan)
  - <alt görev> (+Y puan)
  Bonus:
  - <bonus görev> (+Z puan)

  Kaynaklar:
  - https://...
  - https://...
  ```
- **example_url** — (opsiyonel) kartta canlı önizleme olarak gösterilecek örnek proje linki

> Bu havuzdaki kartlar Cem'e girdi olacak şekilde yazıldı: her kartta senaryo tohumu,
> ana mekanik, learning goal taslağı ve gerçek kaynaklar var. Puanlamayı Cem netleştirecek.

## Çeşitlilik tablosu (domain kapsaması)

Cem'in istediği domain dağılımı. Her proje eklendikçe bu tablo dolacak.

**Tam kart yazılan proje: 27** (+ 12 ek fikir = 39 toplam). Level dağılımı — tam kartlar:
15 `PRESEED` / 12 `SEED` / 0 `SERIES_A`. Ek fikirlerle birlikte: 18 `PRESEED` / 20 `SEED`
/ 1 `SERIES_A`. (PRESEED=Beginner, SEED=Intermediate, SERIES_A=Advanced.) Bilerek geniş
tutuldu — Cem'in "göz çıkarmaz" notuna göre kesip seçebilirsiniz.

> Not: tam kartların yarısı (12/27) şu an Intermediate olarak yazılı — yani havuz henüz
> "çoğunlukla beginner" değil. SEED kartların çoğu bir alt seviyeye sadeleştirilebilir
> (bonus'ları kaldır, tek modele indir vb.).

| Domain | Projeler |
|---|---|
| Gen-media (fal) | Masal Makinesi · Konuşan Tablo · Vitrin · Sesli Not→Sosyal Kart _(+ Jingle Fabrikası, Anı Sineması)_ |
| WebRTC / realtime / voice AI | Tarayıcıda Görüntülü Görüşme · Canlı Altyazı · Sesli AI Karakter · Karaoke Pitch _(+ Fully-Local Voice Assistant)_ |
| Trading / forecasting / memory | Paper-Trading Botu + Günlük · Kripto Fiyat Tahmin Panosu · Öğrenen Harcama Sınıflandırıcı |
| Orkestrasyon (harness / MCP / skill) | Eğlenceli MCP Server · Haftalık Agent Skill _(+ Kripto Fiyat MCP Tool, AgentRace)_ |
| Genel webdev (CRUD) | Link-in-Bio · URL Kısaltıcı · New-Tab Eklentisi · Kanban Panosu · Chat-with-PDF · Weather CLI · FriendKeeper (YC) _(+ Wordle, Uptime Monitor, Değişim İzleyici)_ |
| Mobil (Swift / RN / Expo) | Streak Keeper (Expo) · PocketTutor (Expo+AI) _(+ FlashDeck SwiftUI, AR Oyuncağı)_ |
| Gaming (Unity / Unreal / JS) | Bean Dash (KAPLAY) · Gem Runner (Unity/Phaser) |
| AI agent / vertical SaaS (YC) | RoboHost · SmartSlots · FormChat _(+ GradeBot, MeetingMinutes)_ |

---

<!-- PROJE KARTLARI — araştırma ajanları döndükçe dolduruluyor -->

## WebRTC / Realtime / Voice AI

### Tarayıcıda Görüntülü Görüşme
**Domain:** WebRTC/realtime · **Level:** `PRESEED` (100p) · **Zorluk:** Kolay

**Senaryo tohumu:** Uygulama indirmeden, üyelik olmadan çalışan minik bir "tarayıcıda
FaceTime". İki kişi bir oda kodu paylaşıyor ve birbirini görüyor. Kurulum yok, sunucuda
video saklanmıyor — görüntü doğrudan iki tarayıcı arasında akıyor.

**Ana mekanik:** WebRTC peer connection ve "signaling" problemi. Öğrenci `getUserMedia`
ile kamera/mikrofonu yakalıyor, PeerJS'in ücretsiz hosted broker'ı üzerinden offer/answer
takas ediyor. Ham SDP/ICE detayına girmeden asıl fikri kavrıyor: *tarayıcılar birbirini
bir buluşma sunucusu olmadan bulamaz, ama medya sonra P2P akar.*

**Learning goals (taslak):**
- `navigator.mediaDevices.getUserMedia()` ve tarayıcı izin akışı
- `MediaStream`'i `<video>`'ya bağlamak (yerel önizleme vs. uzak akış)
- Signaling sunucusu ne işe yarar, WebRTC neden ona ihtiyaç duyar
- Peer ID, call/answer akışı, disconnect yönetimi
- Bonus: track enable/disable ile sesi kapatma / kamerayı kapatma

**Kaynaklar:**
- https://github.com/peers/peerjs
- https://peerjs.com/
- https://github.com/Abhi5h3k/WebRTC-PeerJs-Demo

### Canlı Altyazı — "Her Şeye Altyazı"
**Domain:** WebRTC/realtime · **Level:** `PRESEED` (100p, SEED'e uzayan bonus) · **Zorluk:** Kolay→Orta

**Senaryo tohumu:** Bir dersin, toplantının ya da videonun canlı altyazısı. Mikrofona
konuş, ekranda yazı gerçek zamanlı aksın; ya da bir ses dosyası bırak, transcript'i al.

**Ana mekanik:** Mikrofon sesini bir speech-to-text motoruna akıtmak. Tier 1 (kolay):
tarayıcının yerleşik Web Speech API'si, ~20 satır kod. Tier 2 (bonus, orta): Transformers.js
ile Whisper'ı **tarayıcıda** çalıştırmak — gerçek bir sinir ağının ne olduğunu öğretir:
ağırlıkları indirmek, sesi chunk'lamak, inference gecikmesi.

**Learning goals (taslak):**
- Web Speech API: interim vs. final sonuç, sürekli tanıma
- "Streaming" transcription ile batch dosya transcription farkı
- Tarayıcıda gerçek bir sinir ağı (Whisper) çalıştırmak — sunucusuz, gizlilik varsayılan
- Ses chunk'lama, sample rate, model boyutu ↔ gecikme/doğruluk dengesi
- Bonus: zaman damgalı `.srt` altyazı olarak dışa aktarma

**Kaynaklar:**
- https://developer.mozilla.org/en-US/docs/Web/API/Web_Speech_API
- https://github.com/xenova/whisper-web
- https://huggingface.co/spaces/Xenova/whisper-web
- https://github.com/ggml-org/whisper.cpp

### Sesli AI Karakter (Bas-Konuş)
**Domain:** Voice AI · **Level:** `SEED` (400p) · **Zorluk:** Orta

**Senaryo tohumu:** "Bir korsanla / öğretmenle / oyun NPC'siyle konuş" — space'e basılı tut,
sesli soru sor, AI karakter sesli cevap versin. Ev yapımı ChatGPT Voice.

**Ana mekanik:** Klasik voice-agent pipeline'ı: mikrofon → STT → LLM → TTS → ses. Beginner
versiyonu sade olabilir: hem tanıma hem sentez için Web Speech API + tek bir LLM API çağrısı,
mikrofonu bas-konuş butonu ile kapılıyor. Asıl zorluk async pipeline'ı orkestre etmek ve
konuşma sırası/durumu yönetmek. Güçlü öğrenci Python + Pipecat ile yeniden kurabilir.

**Learning goals (taslak):**
- Bas-konuş state machine (boşta → dinliyor → düşünüyor → konuşuyor)
- Async aşamaları zincirleme: tanıma sonucu → API isteği → ses sentezi
- LLM'i karakterde tutacak prompt, sohbet geçmişini saklama
- `SpeechSynthesisUtterance`, ses seçimi, oynatmayı kesme
- Gecikme bütçesi: voice agent neden ~1sn üstünde ölü hissettirir
- Bonus: barge-in (kullanıcı AI'ın üstüne konuşunca AI susuyor)

**Kaynaklar:**
- https://developer.mozilla.org/en-US/docs/Web/API/Web_Speech_API
- https://github.com/pipecat-ai/pipecat
- https://docs.pipecat.ai/overview/introduction
- https://github.com/livekit/agents

### Karaoke Pitch / Enstrüman Akordu
**Domain:** Realtime/DSP · **Level:** `PRESEED` (100p) · **Zorluk:** Orta (API gerektirmez)

**Senaryo tohumu:** Mikrofona şarkı söyle, bir ibre (ya da kayan "pitch izi") notayı tutup
tutmadığını göstersin — gitar akort ve karaoke skor uygulamalarının arkasındaki mekanik.

**Ana mekanik:** Tarayıcıda ham ses DSP'si, Web Audio API. `getUserMedia` → `AudioContext`
→ `AnalyserNode` canlı dalga formu tamponu veriyor; autocorrelation ile temel frekans
çıkıyor; Hz → en yakın nota → cent sapması. ML yok, sunucu yok — sadece örnekler üzerinde
matematik. "Ses" kavramını en çok demistifiye eden proje.

**Learning goals (taslak):**
- Web Audio node grafiği: source → analyser → (opsiyonel) destination
- Zaman-domaini vs. frekans-domaini; dalga formu tamponu neyi içerir
- Autocorrelation pitch detection (kendi yaz, ya da önce `pitchy` kütüphanesi)
- Hz → MIDI nota dönüşümü (`69 + 12·log2(f/440)`) ve cent sapması
- `requestAnimationFrame` ile canvas animasyon döngüsü
- Bonus: karaoke modu — sabit hedef melodiye göre skorlama

**Kaynaklar:**
- https://github.com/cwilso/PitchDetect
- https://cwilso.github.io/PitchDetect/
- https://alexanderell.is/posts/tuner/
- https://github.com/chordbook/tuner

## Mobil & Gaming

### Streak Keeper — Alışkanlık Takipçisi (Expo)
**Domain:** Mobil (Expo/RN) · **Level:** `PRESEED` (100p) · **Zorluk:** Kolay

**Senaryo tohumu:** "Spor / kitap / su içme serimi bozmak istemiyorum" — herkesin tek cümlede
anladığı bir uygulama. Günlük check-off ızgarası + seri (streak) sayacı.

**Ana mekanik:** Asıl ilginç kısım *tarih matematiği* (seri gece bozuldu mu?) ve durumu
lokalde saklayıp uygulama yeniden açılınca serinin hayatta kalması. Expo Go ile öğrenci
kendi telefonunda anında çalıştırıyor — Mac/Xcode gerekmiyor.

**Learning goals (taslak):**
- `npx create-expo-app` ile scaffold, Expo Go ile gerçek telefonda çalıştırma
- Temel RN bileşenleri (FlatList, Pressable) ve flexbox layout
- AsyncStorage ile lokal kalıcılık
- React hook'larıyla basit state yönetimi
- Bonus: seri 7 güne ulaşınca kutlama animasyonu

**Kaynaklar:**
- https://docs.expo.dev/tutorial/introduction/
- https://github.com/machadop1407/react-native-course-habit-tracker
- https://github.com/martinwinther/QuickHabitTracker

### Bean Dash — Tek Tuşlu 2D Web Oyunu (KAPLAY)
**Domain:** Gaming (JS engine) · **Level:** `PRESEED` (100p) · **Zorluk:** Kolay (listedeki en yumuşak giriş)

**Senaryo tohumu:** Arkadaşlarına link olarak yollayabileceğin Flappy-Bird tarzı bir kaçış
oyunu — kurulum yok, her tarayıcıda oynanıyor.

**Ana mekanik:** Tek input (tap/space = zıpla), yerçekimi, kayan engeller, çarpışma = oyun
biter, skor sayacı. Yerçekimi ve boşluk boyutunu ayarlamak "game feel"i hızlı öğretiyor.
KAPLAY (kaboom.js'in devamı) tarayıcıda 30 saniyede başlatılıyor.

**Learning goals (taslak):**
- Game loop düşüncesi: spawn, update, destroy
- Sprite'lar, yerçekimi/fizik alanları, çarpışma callback'leri
- KAPLAY'in bileşen tabanlı entity yapısı (`add([sprite(), pos(), area(), body()])`)
- Sahneler (menü → oyun → game over) ve skor state'i
- Web'e yayınlama (sadece bir HTML sayfası)

**Kaynaklar:**
- https://kaplayjs.com/
- https://docs.kaplayjs.com/guides/
- https://play.kaplayjs.com
- https://github.com/kaplayjs/kaplay

### Gem Runner — 2D Platformer (Unity mod / Phaser)
**Domain:** Gaming (Unity/Phaser) · **Level:** `SEED` (400p) · **Zorluk:** Orta

**Senaryo tohumu:** "Kendi Mario bölümünü yap" — bölüm tasarla, zıplama fiziğini ayarla,
tehlikeler ekle.

**Ana mekanik:** Platformer hareketi (koş/zıpla), toplanabilirler, düşmanlar. Unity track'te
çalışan bir oyunu *modlarsın* (tilemap, oyuncu hızı, yeni bölüm) — bir haftaya gerçekçi.
Web track'te Phaser'ın resmi 10 bölümlük tutorial'ı her şeyi sıfırdan kurar.

**Learning goals (taslak):**
- Unity: editör navigasyonu, tilemap, prefab, C# script ayarları, oynanabilir bölüm build'i
- Phaser: sprite, arcade physics, klavye input, skor + game-over mantığı
- Her iki track'te bölüm tasarımı ve zorluk ayarı

**Kaynaklar:**
- https://learn.unity.com/project/2d-platformer-template
- https://phaser.io/tutorials/making-your-first-phaser-3-game
- https://phaser.io/news/2026/04/phaser-vs-kaplay-vs-excalibur-2d-web-game-framework

### PocketTutor — AI API Çağıran Mobil Uygulama (Expo + AI SDK)
**Domain:** Mobil + AI · **Level:** `SEED` (400p) · **Zorluk:** Orta

**Senaryo tohumu:** Cebinde ödev yardımcısı / quiz üretici — bir konu yaz, akan (streaming)
açıklamalar ya da alıştırma soruları al.

**Ana mekanik:** LLM yanıtlarını token token bir chat UI'ına akıtmak (`useChat` hook'u ağır
işi yapıyor). Bonus: bir "tool" ekle (ör. sözlük/hava durumu) ki model gerçek veri çekebilsin.

**Learning goals (taslak):**
- RN'de dış API çağırma ve async veri yönetimi
- Streaming UI güncellemeleri (neden streaming, tüm yanıtı beklemekten iyi)
- API key'leri client kodundan uzak tutmak (env var / API route)
- Prompt tasarımı: system prompt tutor'un kişiliğini şekillendirir

**Kaynaklar:**
- https://ai-sdk.dev/docs/getting-started/expo
- https://docs.expo.dev/agents/
- https://designcode.io/react-native-ai/

## Trading / Forecasting / Memory & Orkestrasyon

### Paper-Trading Botu + İşlem Günlüğü Hafızası
**Domain:** Trading + memory · **Level:** `SEED` (400p) · **Zorluk:** Orta

**Senaryo tohumu:** "Botum aynı formasyonda üç kez para kaybetti — ona geçmiş işlemlerinin
hafızasını verdim ve almadan önce günlüğüne bakmasını sağladım."

**Ana mekanik:** Döngü: fiyat çek → basit kural uygula (ör. hareketli ortalama kesişimi) →
paper order vermeden önce o sembolde geçmiş işlemlerin günlüğünü (SQLite/Mem0) sorgula →
Alpaca paper API ile emir ver → kapanışta sonucu + tek satır "ders"i günlüğe yaz. İlginç
kısım feedback loop: botun bu geceki davranışı geçen hafta olana bağlı.

**Learning goals (taslak):**
- Gerçek bir broker REST API'sini güvenle çağırmak (paper hesap, API key, env var)
- Basit bir strateji (SMA crossover) ve neden başarısız olduğu
- Kalıcılık: küçük bir işlem günlüğü şeması (giriş, çıkış, PnL, sebep)
- Aksiyondan-önce-retrieval: agent hafızasının en basit hali
- Bir script'i zamanlama (cron veya while-loop)

**Kaynaklar:**
- https://docs.alpaca.markets/us/docs/paper-trading
- https://alpaca.markets/sdks/python/
- https://github.com/mem0ai/mem0
- https://github.com/kernc/backtesting.py

### Kripto Fiyat Tahmin Panosu
**Domain:** Forecasting · **Level:** `SEED` (400p) · **Zorluk:** Kolay→Orta

**Senaryo tohumu:** "Bitcoin'in son 90 gününü ve dürüst bir belirsizlik bandıyla 7 günlük
tahminini gösteren canlı bir web sayfası."

**Ana mekanik:** CoinGecko ücretsiz API'sinden günlük geçmiş fiyatları çek, StatsForecast ile
2-3 klasik model (AutoARIMA, AutoETS, Theta) fit et, tahminleri prediction interval'la
Streamlit/Plotly panosunda çiz. Öğretici an: held-out günlerde backtest edip naive "dünkü
fiyat" baseline'ını yenmenin zor olduğunu göstermek — forecasting hakkında dürüst bir ders.

**Learning goals (taslak):**
- Rate-limitli public JSON API tüketmek
- Time-series temelleri: zamanda train/test split, neden shuffle yapılmaz
- Forecasting modelleri fit etme/karşılaştırma; prediction interval okuma
- Değerlendirme metrikleri (MAE, MAPE) vs. naive baseline
- Basit bir veri panosu kurmak

**Kaynaklar:**
- https://docs.coingecko.com/reference/introduction
- https://github.com/Nixtla/statsforecast
- https://github.com/ranaroussi/yfinance

### Öğrenen Harcama Sınıflandırıcı
**Domain:** Data + memory · **Level:** `PRESEED` (100p) · **Zorluk:** Kolay

**Senaryo tohumu:** "Bankanın CSV export'unu bırak, her işlemi etiketlesin — bir etiketi bir
kez düzeltince o mağazayı bir daha asla yanlış yapmasın."

**Ana mekanik:** İki katmanlı sınıflandırma: önce öğrenilmiş kural deposunu (mağaza→kategori)
kontrol et, bilinmeyende LLM (ya da API'siz sürüm için keyword kuralları) çağır. Her manuel
düzeltme depoya geri yazılır. Memory-as-personalization: sistem kullandıkça gözle görülür
şekilde iyileşiyor — ilk proje için çok motive edici.

**Learning goals (taslak):**
- Gerçek dünya CSV'lerini pandas ile parse etmek
- Öncelik deseni: birebir hafıza > model tahmini
- Basit LLM structured-output prompting (sabit listeden tek kategori)
- Basit kalıcılık (JSON/SQLite) ve aylık harcama özet grafiği

**Kaynaklar:**
- https://github.com/mem0ai/mem0
- (Sadece pandas + SQLite ile API'siz de yapılabilir — en güvenli 1 haftalık proje)

### Eğlenceli Bir MCP Server (Claude'a Yeni Bir Tool Ver)
**Domain:** Orkestrasyon (MCP) · **Level:** `PRESEED` (100p) · **Zorluk:** Kolay (küçük yüzey, büyük etki)

**Senaryo tohumu:** "Herhangi bir AI asistanının hileli zar atmasını / canlı kripto fiyatı
çekmesini / playlist'imi puanlamasını sağlayan bir server yazdım — Claude'a tek satır config
ile takılıyor."

**Ana mekanik:** Model Context Protocol'ün kendisi: tools/resources/prompts sunan bir JSON-RPC
server; herhangi bir MCP host'u (Claude Desktop, Claude Code) keşfedip çağırabiliyor. FastMCP
ile çalışan bir tool tam anlamıyla decorate edilmiş bir Python fonksiyonu — şema, validation,
transport senin için üretiliyor.

**Learning goals (taslak):**
- Protokol nedir: capability negotiation, tool discovery, JSON-RPC
- LLM'in gerçekten kullanabileceği tool açıklamaları yazmak (docstring = UX)
- stdio vs HTTP transport
- Gerçek bir host'a karşı test etmek ve resmi reference server'ları okumak

**Kaynaklar:**
- https://modelcontextprotocol.io/specification/2025-06-18
- https://github.com/jlowin/fastmcp
- https://github.com/modelcontextprotocol/python-sdk
- https://github.com/modelcontextprotocol/servers

### Haftalık İş Akışını Otomatikleştiren Agent Skill
**Domain:** Orkestrasyon (skills) · **Level:** `SEED` (400p) · **Zorluk:** Kolay→Orta

**Senaryo tohumu:** "`/market-report` yaz, agent bu haftanın fiyatlarını çeksin, getirileri
hesaplasın ve biçimli bir Markdown brifing yazsın — çünkü prosedürü ona bir kez öğrettin."

**Ana mekanik:** Anthropic Agent Skills deseni: `SKILL.md` (YAML frontmatter + adım adım
talimat) + yardımcı script'ler içeren bir klasör, agent tarafından gerektiğinde yükleniyor.
Öğrenci *prosedürü* yazıyor (veri çek → haftalık istatistik hesapla → rapor şablonu doldur),
agent tekrarlanabilir şekilde çalıştırıyor. Doğal bir capstone — başka projenin kodunu
yeniden kullanabilir.

**Learning goals (taslak):**
- Skills deseni: progressive disclosure, frontmatter metadata, bundled script
- Agent'ın deterministik takip edebileceği kadar kesin talimat yazmak
- Skill'i küçük doğrulanmış script'lerden kurmak vs. serbest LLM çıktısı
- Skill vs. MCP tool: talimat vs. protokol seviyesi capability

**Kaynaklar:**
- https://github.com/anthropics/skills
- https://platform.claude.com/docs/en/agents-and-tools/agent-skills/overview
- https://github.com/ranaroussi/yfinance

## Genel Webdev (CRUD)

### Link-in-Bio Sayfası ("Bedava Linktree")
**Domain:** Webdev (statik→CRUD) · **Level:** `PRESEED` (100p) · **Zorluk:** Kolay (ideal hafta-1 projesi)

**Senaryo tohumu:** Linktree, aslında tek bir stilize buton sayfası için aylık $5-24 alıyor.
Instagram bio'su olan herkes görmüştür.

**Ana mekanik:** Tek responsive profil sayfası (avatar, bio, buton listesi), sonra seviye
atla: link'leri JSON/SQLite'a yazan minik admin form ve link başına tıklama sayacı.

**Learning goals (taslak):**
- HTML/CSS layout, flexbox, mobile-first tasarım
- Statik site deploy (GitHub Pages / Netlify / Vercel)
- JSON'dan config okumak → veri ile sunumu ayırmak
- Bonus: tıklama sayacını artıran ilk backend route'u

**Kaynaklar:**
- https://github.com/sethcottle/littlelink

### URL Kısaltıcı + Tıklama Analitiği ("Bedava Bitly")
**Domain:** Webdev (backend CRUD) · **Level:** `SEED` (400p) · **Zorluk:** Orta (ilk "gerçek backend")

**Senaryo tohumu:** Bitly ücretli planlara ~$10/ay ile başlıyor; Dub tam bu fikir üstüne
kurulmuş VC destekli bir startup.

**Ana mekanik:** Uzun URL POST et → kısa slug üret → eşlemeyi sakla → HTTP redirect route.
"Aha" anı: redirect aslında bir 302 yanıtı, analitik ise `clicks += 1` + timestamp tablosu.

**Learning goals (taslak):**
- REST route'ları ve HTTP redirect/status kodları
- Veritabanı temelleri (bir-iki tablo, SQLite yeter)
- Slug üretimi, çakışma yönetimi, input validation
- Tıklama sayılarını gösteren minik dashboard sayfası

**Kaynaklar:**
- https://github.com/dubinc/dub

### Özel New-Tab Panosu (Tarayıcı Eklentisi, "Bedava Momentum")
**Domain:** Webdev (browser extension) · **Level:** `PRESEED` (100p) · **Zorluk:** Kolay (backend yok)

**Senaryo tohumu:** Momentum'un şık yeni-sekme sayfasının ücretli tier'ı var; Tabliss sevilen
bedava versiyon. Kendi eklentini Chrome'a kurmak büyük bir motivasyon anı.

**Ana mekanik:** Tarayıcı eklentisi aslında bir web sayfası + `manifest.json`. Yeni sekmeyi
saat, selamlama, arkaplan görseli, hava durumu (API'den) ve `localStorage`'a kaydedilen todo
listesiyle değiştir.

**Learning goals (taslak):**
- WebExtension anatomisi: manifest v3, izinler, paketleme
- DOM manipülasyonu ve `localStorage`
- Public API çağırmak (hava durumu, alıntı, Unsplash)
- Gerçek tarayıcıya yayınlama (load-unpacked → opsiyonel store)

**Kaynaklar:**
- https://github.com/joelshepherd/tabliss
- https://github.com/BookCatKid/TablissNG

### Kanban Panosu ("Bedava Trello")
**Domain:** Webdev (frontend-ağırlıklı CRUD) · **Level:** `SEED` (400p) · **Zorluk:** Orta (klasik capstone)

**Senaryo tohumu:** Trello/Notion panoları abonelik ürünü; kaneo bu hafta GitHub trending'deydi
— talep gözle görülür şekilde güncel.

**Ana mekanik:** Sütunlar + kartlar + **sürükle-bırak** — bir beginner'ın gönderebileceği en
etkileyici etkileşim. Önce `localStorage`'a, sonra gerçek backend'e kalıcı.

**Learning goals (taslak):**
- Uygulama state modelleme (board → column → card)
- Sürükle-bırak event'leri ve yeniden sıralama mantığı
- localStorage'a karşı CRUD, sonra aynı uygulamayı API + DB'ye yükseltme
- Optimistic UI güncellemeleri

**Kaynaklar:**
- https://github.com/usekaneo/kaneo
- https://github.com/excalidraw/excalidraw

### PDF'inle Sohbet Et ("Bedava ChatPDF")
**Domain:** Webdev + AI (RAG) · **Level:** `SEED` (400p) · **Zorluk:** Orta

**Senaryo tohumu:** ChatPDF ve düzinelerce klonu "PDF yükle, sorularını sor" için abonelik
alıyor. Programcı olmayan arkadaşların en çok istediği demo — bu yüzden harika bir portfolyo
parçası.

**Ana mekanik:** PDF metnini çıkar → chunk'la → ilgili chunk'ları embed/retrieve et → LLM
prompt'una koy → chat UI (Streamlit ile ~10 satır). RAG kavramı, ML eğitimi yok.

**Learning goals (taslak):**
- LLM API'sini güvenle çağırmak (key'ler env var'da, asla git'te değil)
- RAG kavramı: chunking, retrieval, context window
- Dosya upload ve PDF metin çıkarma
- Streamlit ile hızlı UI; token/maliyet limitleri (Ollama ile lokal = bedava)

**Kaynaklar:**
- https://github.com/Shubhamsaboo/awesome-llm-apps

### Terminal Hava Durumu / Tek-Atış API CLI ("wttr.in tarzı")
**Domain:** Webdev (CLI + API) · **Level:** `PRESEED` (100p) · **Zorluk:** Kolay (en hızlı dopamin, git pratiği)

**Senaryo tohumu:** `curl wttr.in` günde ~100M istek karşılıyor — basit bir "API gir, terminale
güzel çıktı" aracının internet-ünlüsü olabileceğinin kanıtı.

**Ana mekanik:** Bir CLI (`weather istanbul`, `crypto btc`, `github-stats <user>`) ücretsiz
API'ye vurup JSON parse edip renk/ASCII ile render ediyor. npm/PyPI'a yayınla ki sınıf
arkadaşların `pip install` edebilsin.

**Learning goals (taslak):**
- Argüman parse (argparse / commander)
- JSON API tüketmek; API hatalarını ve eksik veriyi yönetmek
- Terminal biçimlendirme (ANSI renk, tablo — Python'da `rich` sihirli)
- Gerçek kurulabilir bir araç paketleyip yayınlamak

**Kaynaklar:**
- https://github.com/chubin/wttr.in

## Gen-Media (fal partnerliği)

> Hepsi tek ve aynı SDK çağrısını kullanıyor: `fal.subscribe(endpoint, {input})`. Öğrenci
> bunu 1. projede bir kez öğreniyor, sonra sadece endpoint string'i ve input şeması değişiyor.
> **1. gün proxy'yi öğret:** `@fal-ai/server-proxy` Next.js deseni, paylaşılan akademi
> `FAL_KEY`'inin tarayıcıya sızmasını engelliyor.
> Kurulum/quickstart: https://docs.fal.ai/model-apis/quickstart · Proxy:
> https://docs.fal.ai/model-apis/integrations/nextjs · Model kataloğu: https://fal.ai/explore/models

### Masal Makinesi — Resimli Sesli Hikâye Kitabı
**Domain:** Gen-media (image + TTS) · **Level:** `PRESEED` (100p) · **Zorluk:** Kolay

**Senaryo tohumu:** Küçük kardeşin için 5 cümlelik bir masal yaz; geri dönüşte aileye
yollayabileceğin, seslendirilmiş, resimli bir çevirmeli kitap web sayfası al.

**Ana mekanik:** *Tutarlılık mühendisliği.* Öğrenci tek bir "karakter sayfası" prompt öneki
yazıyor ve her sayfanın image prompt'una programatik olarak ekliyor ki kahraman tüm FLUX
üretimlerinde aynı görünsün — prompt'u yazmak değil, kod olarak template'lemenin ilk tadı.
Sayfalar paralel `fal.subscribe` çağrılarıyla açılıyor; seslendirme sayfa başına bir TTS
çağrısından geliyor, sayfa çevirmelerine `<audio>` `ended` event'iyle senkronlanıyor.

**Learning goals (taslak):**
- JSON HTTP API çağırmak ve yapısal yanıt okumak (`result.data.images[0].url`)
- String templating / prompt'ları programatik kurmak
- Paralel üretim için `Promise.all` vs. sıralı await
- API key'i Next.js proxy route arkasına saklamak
- Temel audio/DOM event yönetimi

**Kaynaklar:**
- https://fal.ai/models/fal-ai/flux-2-pro
- https://fal.ai/models/fal-ai/elevenlabs/tts/multilingual-v2
- https://github.com/fal-ai/fal-nextjs-template

### Konuşan Tablo — Müze Tarzı Konuşan Portreler
**Domain:** Gen-media (TTS + video + lipsync) · **Level:** `SEED` (400p) · **Zorluk:** Orta

**Senaryo tohumu:** Herhangi bir portre yükle (tarihi figür, kedin, kendi çizimin) ve ne
söylemesini istediğini yaz — Harry Potter tablosu gibi geri konuşsun.

**Ana mekanik:** *Model zincirleme.* Bir modelin çıktısı diğerinin girdisi oluyor: metin →
TTS ses URL → image-to-video klip → lipsync ikisini birleştiriyor. Öğrenci generative
pipeline'ların aslında endpoint'ler arası URL taşımak olduğunu öğreniyor ve gerçek queue
süresiyle karşılaşıyor (video dakikalar sürer → donuk spinner yerine `onQueueUpdate` ile
ilerleme UI'ı kurmalı).

**Learning goals (taslak):**
- Çıktı URL'lerinin sonraki isteği beslediği async zincir
- Uzun işleri yönetmek: queue durumu, polling, optimistic UI
- Tarayıcıdan dosya upload ve hosted URL'i API'ye geçirmek
- Maliyet farkındalığı: tıklamadan önce üretim başına krediyi tahmin etmek

**Kaynaklar:**
- https://fal.ai/models/fal-ai/elevenlabs/tts/eleven-v3
- https://fal.ai/models/fal-ai/kling-video/v2.1/standard/image-to-video
- https://fal.ai/models/veed/lipsync
- https://github.com/fal-ai-community/video-generator-demo

### Vitrin — Tek Dokunuşluk Ürün Fotoğraf Stüdyosu
**Domain:** Gen-media (image pipeline) · **Level:** `PRESEED` (100p) · **Zorluk:** Kolay→Orta

**Senaryo tohumu:** Teyzen Instagram'da el yapımı takı satıyor; mutfak masasındaki telefon
fotoğrafları tek dokunuşla stüdyo kalitesinde katalog çekimine dönüşüyor.

**Ana mekanik:** *Her aşamada kullanıcı seçimi olan üç aşamalı image pipeline:* BiRefNet
arkaplanı soyuyor → nano-banana `/edit` ürünü 4 hazır sahne stilinden birine yerleştiriyor →
Clarity kazananı upscale ediyor. İlginç kısım karşılaştırma UI'ı: orijinal vs. her aşama yan
yana; öğrenci pipeline düşünmeyi ve tek üretime güvenmek yerine kullanıcıya aday sunmayı
öğreniyor.

**Learning goals (taslak):**
- Image pipeline tasarımı (her model bir işi iyi yapar)
- Prompt tabanlı editing vs. mask tabanlı editing (nano-banana neden mask'sız)
- N aday üretim sunmak ve seçim state'i yönetmek
- Çok modelli pipeline'da çağrı başına fiyatlandırmayı anlamak

**Kaynaklar:**
- https://fal.ai/models/fal-ai/birefnet/v2
- https://fal.ai/models/fal-ai/nano-banana-2/edit
- https://fal.ai/models/fal-ai/clarity-upscaler
- https://github.com/fal-ai-community/infinite-kanvas

### Sesli Not → Sosyal Kart — Podcast Alıntı Makinesi
**Domain:** Gen-media (STT + image) · **Level:** `SEED` (400p) · **Zorluk:** Kolay→Orta

**Senaryo tohumu:** Haftan hakkında 60 saniyelik sesli not konuş; uygulama transkript etsin,
en iyi cümleyi seçsin ve Instagram'a hazır tasarlanmış bir alıntı kartı + audiogram üretsin.

**Ana mekanik:** *Tam medya turu: ses girer → metin → görsel çıkar.* Kayıt tarayıcının
`MediaRecorder`'ından geliyor, Wizper kelime zaman damgalarıyla transkript ediyor, öğrenci
"alıntılanabilir" cümleyi seçmek için gerçek string-processing mantığı yazıyor (uzunluk/keyword
sezgileri — ML'siz gerçek algoritmik düşünce), sonra bunu FLUX prompt template'ine besliyor.

**Learning goals (taslak):**
- Tarayıcı medya yakalama (`MediaRecorder`, blob, upload)
- Speech-to-text API'leri ve transkript/timestamp verisiyle çalışmak
- Metin üzerinde seçim sezgileri yazmak (ilk "ürüne giden algoritma")
- Canvas/CSS ile görsel üstüne metin kompozisyonu
- Bonus: sadece-kendi-sesi kuralıyla sorumlu voice cloning

**Kaynaklar:**
- https://fal.ai/models/fal-ai/wizper
- https://docs.fal.ai/examples/model-apis/convert-speech-to-text
- https://fal.ai/models/fal-ai/minimax/voice-clone

## YC-esintili Projeler (son batch'lerden basitleştirilmiş)

### RoboHost — Restoran Sipariş Chatbot'u (YC S25: Certus AI)
**Domain:** AI agent · **Level:** `PRESEED` (100p, sesli bonusla SEED) · **Zorluk:** Kolay (sesle Orta)

**Senaryo tohumu:** "Yakınındaki bir restoran muhtemelen telefonuna zaten böyle bir AI ile
cevap veriyor." Menü (JSON), chat penceresi ve menüyle prompt'lanmış bir LLM sohbet ederek
sipariş alıyor, sonra yapısal sipariş özeti (ürünler, adet, toplam) çıkarıyor.

**Ana mekanik:** LLM'i yapısal veriyle (menü) system-prompt'lamak ve yapısal çıktı (sipariş
JSON) zorlamak — telefonu çıkarınca her "voice agent" startup'ının özü. Bonus: Web Speech API
ile sesli giriş/çıkış.

**Learning goals (taslak):**
- Backend route'tan LLM API çağırmak
- System prompt + uygulama verisini prompt'a enjekte etmek
- Modelden yapısal JSON almak ve doğrulamak
- Temel chat UI state (mesaj geçmişi)
- Bonus: sesli giriş/çıkış için Web Speech API

**Kaynaklar:**
- https://www.ycombinator.com/companies/certus-ai
- https://developer.mozilla.org/en-US/docs/Web/API/Web_Speech_API

### FriendKeeper — Kişisel CRM (YC S25: Pally)
**Domain:** Webdev (klasik CRUD) · **Level:** `PRESEED` (100p) · **Zorluk:** Kolay (ideal ilk full-stack DB app)

**Senaryo tohumu:** "Bu kadar basit bir CRM Y Combinator'dan para aldı — AI bir özellik, asıl
ürün veritabanı." Etiket ve notlarla kişi ekle, etkileşim kaydet, "X ile 30 gündür
konuşmadın" hatırlatması gör.

**Ana mekanik:** Saf ilişkisel CRUD + tarih matematiği ("son temastan bu yana geçen gün") —
ideal ilk full-stack veritabanı uygulaması. Bonus: notlara göre check-in mesajı taslaklayan
tek LLM butonu.

**Learning goals (taslak):**
- Veritabanı şema tasarımı (contacts ↔ interactions, bire-çok)
- Tam CRUD route'ları ve formlar
- Tarih aritmetiği, sıralama/filtreleme
- Bonus: basit auth / kullanıcı başına veri; mesaj taslağı için tek LLM çağrısı

**Kaynaklar:**
- https://www.ycombinator.com/companies/pally

### SmartSlots — Dinamik Fiyatlama Motoru (YC W26: Booko)
**Domain:** Algoritma + CRUD (AI'siz) · **Level:** `PRESEED` (100p) · **Zorluk:** Kolay

**Senaryo tohumu:** "Uber'in surge-pricing matematiği, mahalledeki spor salonuna uygulanmış —
ve YC bunu fonladı." Kurgusal yoga stüdyosu için booking ızgarası (7 gün × 10 slot); her
slotun fiyatı öğrencinin yazdığı bir kuralla hesaplanıyor.

**Ana mekanik:** Saf bir fiyatlama fonksiyonu `price(slot, bookings) → sayı` — öğrenci
fonlanabilir bir startup'ın özünün iyi seçilmiş bir formül + CRUD olabileceğini görüyor. AI
bağımlılığı olmadan algoritmik düşünce için harika.

**Learning goals (taslak):**
- 2 boyutlu bir program veri yapısı modellemek
- Fiyatlama fonksiyonu yazmak ve ayarlamak (çarpanlar, clamping)
- State güncellemeleri: rezervasyonlar gösterilen fiyatı canlı değiştiriyor
- İş mantığını UI'dan ayırmak

**Kaynaklar:**
- https://www.ycombinator.com/companies/booko

### FormChat — Konuşan Form (YC S25: RowFlow)
**Domain:** AI agent (state machine) · **Level:** `SEED` (400p) · **Zorluk:** Orta

**Senaryo tohumu:** "Startup'lar sıkıcı web formunun öldüğüne bahse giriyor — sen onun yerine
geçecek şeyi kuracaksın." Formu JSON şema olarak tanımla (isim, email, en sevdiğin pizza,
bütçe…); alanları render etmek yerine LLM her alan dolana dek chat'te görüşme yapıyor,
giderken doğruluyor ve sonunda tamamlanmış yapısal kaydı gösteriyor.

**Ana mekanik:** State'li bir agent loop: model hangi şema alanlarının hâlâ boş olduğunu
takip etmeli, onları doğal sorular ile istemeli ve yapısal veri üretmeli — hâlâ beginner
boyutunda ama gerçekten ilginç bir control-flow problemi.

**Learning goals (taslak):**
- Bir formu JSON şema olarak temsil etmek
- Turn'ler arası konuşma + tamamlanma state'i tutmak
- Serbest metinden tool-tarzı yapısal çıkarım
- Input validation (email formatı, sayılar)
- UX karşılaştırması: form vs. konuşma

**Kaynaklar:**
- https://www.ycombinator.com/companies/rowflow

## Ek fikirler (kart yazılmadı, havuzu genişletmek için)

Aşağıdakiler doğrulanmış kaynaklarıyla hazır; istenirse tam karta çevrilir:

- **Fully-Local Voice Assistant** — sunucusuz "Jarvis": whisper.cpp + Piper/Kokoro TTS. `SERIES_A`, ileri. (https://github.com/OHF-Voice/piper1-gpl · https://huggingface.co/hexgrad/Kokoro-82M)
- **Wordle Klonu** — 5-harf kelime, renkli feedback, klavye state. `PRESEED`, kolay. (https://github.com/topics/wordle-clone)
- **Uptime Monitor + Status Page** — ping döngüsü + Discord webhook. `SEED`. (https://github.com/louislam/uptime-kuma)
- **Fiyat-Düşüş / Sayfa Değişim İzleyici** — scraping + diff + alarm. `SEED`. (https://github.com/dgtlmoon/changedetection.io)
- **Jingle Fabrikası** — form → müzik prompt + TTS tagline, Web Audio ile mix. `SEED`, gen-media. (https://fal.ai/models/fal-ai/minimax-music · https://fal.ai/models/fal-ai/lyria2)
- **Anı Sineması** — eski aile fotoğrafları: upscale → image-to-video → reel, batch job queue. `SEED`, gen-media. (https://github.com/fal-ai-community/video-starter-kit)
- **Tap-to-Place AR Oyuncağı** — ARKit + RealityKit, masaya 3D nesne bırak. `SEED`, mobil (iPhone gerekir). (https://www.createwithswift.com/creating-an-augmented-reality-app-in-swiftui-using-realitykit-and-arkit/)
- **FlashDeck** — SwiftUI + SwiftData flashcard, swipe gesture. `SEED`, mobil (Mac gerekir). (https://www.hackingwithswift.com/books/ios-swiftui/flashzilla-introduction)
- **MeetingMinutes** — ses → transkript → özet/aksiyon (YC: Hyprnote, açık kaynak). `SEED`. (https://github.com/fastrepl/hyprnote)
- **AgentRace** — aynı prompt'u iki modele paralel gönder, yan yana karşılaştır (YC: Emdash). `PRESEED`. (https://github.com/generalaction/emdash)
- **GradeBot** — vision LLM ile el yazısı ödev notlama + sınıf panosu (YC: Frizzle). `SEED`. (https://www.ycombinator.com/companies/frizzle)
- **Kripto Fiyat MCP Tool** — MCP server'ın CoinGecko sarması (Proje "MCP Server" varyantı). `PRESEED`. (https://github.com/jlowin/fastmcp)
