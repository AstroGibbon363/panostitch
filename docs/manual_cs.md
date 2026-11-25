# Panorama Stitcher: uživatelská příručka

*Další jazyky: [English](manual_en.md) · [Deutsch](manual_de.md) · [Español](manual_es.md) · [Français](manual_fr.md) · [Polski](manual_pl.md)*

Program pro počítač, který spojí překrývající se snímky do jedné panoramy nebo
mozaiky: JPEG, TIFF, PNG, RAW z fotoaparátu a astronomické snímky FITS. Každý
krok proběhne až po stisknutí příslušného tlačítka a každý výsledek si můžete
před pokračováním prohlédnout a opravit. Druhá karta skládá snímky hvězd do
oblohy s nízkým šumem nebo do hvězdných stop ([Skládání hvězd](#13-skládání-hvězd)).

Ovládací prvky jsou v této příručce uvedeny **tučně**, tak jak je uvidíte na
obrazovce v češtině. Jazyk okna (čeština, angličtina, němčina, španělština,
francouzština, polština) se přepíná vpravo
nahoře; tato příručka je v programu i pod tlačítkem **? Nápověda** (F1).

---

## Obsah

1. [Požadavky](#1-požadavky)
2. [Spuštění programu](#2-spuštění-programu)
3. [Okno na první pohled](#3-okno-na-první-pohled)
4. [Rychlý začátek](#4-rychlý-začátek)
5. [Krok za krokem](#5-krok-za-krokem)
6. [Pohyb v náhledu](#6-pohyb-v-náhledu)
7. [Projekce](#7-projekce)
8. [Soubory RAW](#8-soubory-raw)
9. [Soubory FITS (astronomie)](#9-soubory-fits-astronomie)
10. [Ukládání](#10-ukládání)
11. [Řešení problémů](#11-řešení-problémů)
12. [Tipy pro dobré panoramy](#12-tipy-pro-dobré-panoramy)
13. [Skládání hvězd](#13-skládání-hvězd)
14. [Použití bez okna](#14-použití-bez-okna)
15. [Soubory](#15-soubory)
16. [Dodatečné úpravy](#16-dodatečné-úpravy)
17. [Timelapse](#17-timelapse)

---

## 1. Požadavky

- **Nic se neinstaluje.** Program je jediný soubor, `panostitch`
  (`panostitch.exe` na Windows), a k tomu `panostack` pro skládání hvězd z
  příkazové řádky. Na Linuxu potřebuje grafický ovladač s OpenGL a X11 nebo
  Wayland, což má každé grafické prostředí.
- **Soubory RAW z fotoaparátu** nepotřebují nic dalšího: dekodér RAW je
  součástí programu. Pokud máte nainstalovaný `dcraw_emu` z LibRaw,
  RawTherapee, darktable nebo dcraw, program je umí použít také (viz
  [Soubory RAW](#8-soubory-raw)).
- **Soubory FITS a XISF** nepotřebují nic dalšího.
- **Dialogy výběru souborů** nepotřebují na Windows a macOS nic navíc. Na
  Linuxu používají `zenity`, `kdialog` nebo `yad`, podle toho, co je
  nainstalované (většina prostředí jeden z nich má). Bez nich zadáváte cesty
  do pole pro cestu.

Pro sestavení programu ze zdrojového kódu potřebujete Rust
(<https://rustup.rs>): `cargo build --release` vytvoří
`target/release/panostitch` a `target/release/panostack`.

---

## 2. Spuštění programu

Poklepejte na `panostitch`, nebo ho spusťte z terminálu:

```
panostitch
```

Snímky nebo soubor projektu můžete rovnou předat k otevření:

```
panostitch ~/Pictures/pano/*.NEF
panostitch moje_pano.panoproj
panostitch noc.stackproj            # otevře se na kartě skládání hvězd
panostitch noc.tlproj               # otevře se na kartě Timelapse
```

Poznámky:

- Program automaticky využívá jádra procesoru. Dlouhé úlohy jich používají
  nejvýš 85 % a nechávají 15 % paměti volných, takže počítač zůstane
  použitelný.
- Okno se otevře do jedné až dvou sekund; nic se předem nepřipravuje.
- Pokud se okno neobjeví, spusťte program z terminálu: vypíše tam chybová
  hlášení.
- Přepínače vzhledu (Dark, Modern, Simple) se ukládají do `ui.toml` ve složce
  nastavení: `~/.config/panostitch` na Linuxu,
  `~/Library/Application Support/panostitch` na macOS,
  `%APPDATA%\panostitch` na Windows.
- **Jazyk:** okno mluví česky, anglicky, německy, španělsky, francouzsky nebo
  polsky; nabídka vpravo nahoře
  přepne hned (první spuštění se řídí jazykem systému).
- **Nápověda:** **? Nápověda** vpravo nahoře (nebo F1) otevře tuto příručku
  přímo v programu, s obsahem, hledáním a odkazy; F1 otevře kapitolu zobrazené
  karty.
- **Pamatuje si mezi spuštěními:** kartu, na které jste byli, nastavení
  skládání hvězd, nastavení videa v Timelapse (deflicker, pohyb kamery,
  výstup, RAW via) a nastavení RAW ve skládání panoramat. Soubory, masky a
  vzhled si nepamatuje; ty drží soubor projektu.
- **Přetažení myší:** obrázky nebo složku přetažené do okna přidá na
  zobrazenou kartu (Post-processing otevře první obrázek). Přetažený soubor
  projektu se otevře na své kartě.
- **Nedávné projekty** (horní lišta) ukazuje naposledy otevřené nebo uložené
  projekty všech druhů; kliknutím se projekt otevře na své kartě.
- Dialogy souborů začínají ve složce, kterou jste použili naposledy.
- Po uložení otevře **Ukázat ve složce** (ve zprávě, nebo vedle Save v
  Post-processing) složku ve správci souborů.

---

## 3. Okno na první pohled

```
┌──────────────────────┬──────────────────────────────────────┬─────────────────────┐
│ [Open…] [Save proj.] │ [Image+keypoints] [Pair matches]     │ 3 · Projection      │
│ Images               │ [Panorama] [Zoom +] [Zoom -] [Fit]   │  nabídka projekce   │
│  Add Remove Clear ▲▼ │ (o) overlays  [Undo] [Redo]          │  Focal, Yaw, Pitch, │
│  pole pro cestu      │                                      │  Roll               │
│  seznam snímků       │                                      │ 4 · Compose         │
│  Reference           │            hlavní náhled             │  Blend, Preview size│
│  RAW via, half size  │                                      │  Exposure,Brightness│
│  Preset              │                                      │  show unlinked      │
│ 1 · Features         │                                      │  Auto crop, Uncrop  │
│                      │                                      │  Nudge ← ↑ ↓ →      │
│ 2 · Matching         │                                      │ 5 · Stretch         │
│  Pairs, Model, Pair  │                                      │ histogram, posuvníky│
│  podrobnosti páru    │  stavový řádek (červený text)        │ [Preview] [Save…]   │
└──────────────────────┴──────────────────────────────────────┴─────────────────────┘
```

- **Levý sloupec:** otevírání a ukládání projektů, výběr snímků, hledání význačných bodů, párování snímků.
- **Uprostřed:** hlavní náhled a tlačítka pro přepínání pohledů, zoom,
  skrytí překryvů a zpět / znovu.
- **Pravý sloupec:** projekce, expozice, prolínání, jas, ruční umístění,
  roztažení histogramu, náhled a uložení.
- **Červený stavový řádek** (pod náhledem): co program právě dělá, výsledky a
  chybová hlášení.

Přepínače vpravo nahoře mění vzhled: **Tmavý**, **Moderní** (zaoblenější,
vzdušnější ovládání) a **Jednoduchý**. Jednoduchý režim skryje podrobné
ovládání (nastavení hledání a párování bodů, prolínání, roztažení, posouvání)
a ponechá, co potřebuje většina panoram: snímky, předvolbu, **Spustit všechny kroky**,
projekci s ohniskovou vzdáleností, natočením (yaw), sklonem (pitch) a
klopením (roll), expozici, **Vyrovnat**, ořez, náhled a uložení. Skrytá
nastavení si ponechají hodnoty z předvolby. Program si přepínače pamatuje.

---

## 4. Rychlý začátek

**Fotografická panorama:**

1. Nechte zapnuté **poloviční velikost**. Je to rychlejší a na kontrolu výsledku to
   bohatě stačí.
2. Zvolte **Předvolba** podle námětu (viz [Předvolby](#50-předvolby)).
3. Napište složku se snímky do pole pro cestu a stiskněte **Enter**, nebo
   klikněte na **Přidat…** a snímky vyberte. **Na pořadí nezáleží.**
4. Klikněte na **Spustit všechny kroky** a počkejte, až stavový řádek ukáže *Done*.
5. Vpravo zvolte **projekci**: **válcová** pro jednu řadu, **sférická**
   pro více řad nebo pro záběry sahající hodně nahoru či dolů. Je-li výsledek
   tmavý, klikněte na **Automatická expozice**; je-li horizont prohnutý, upravte
   **Sklon**. Tlačítkem **Automatický ořez** odříznete zubaté okraje.
6. Klikněte na **Uložit panorama…**. Chcete-li se k práci později vrátit,
   klikněte i na **Uložit projekt…**.

**Mozaika hlubokého vesmíru ze souborů FITS:**

1. Zvolte předvolbu **deep-sky mozaika**.
2. Načtěte dlaždice FITS (složené nebo jednotlivé snímky). Pro zobrazení se
   automaticky roztáhnou.
3. Klikněte na **Spustit všechny kroky** a potom na **Automatické roztažení**, aby roztažení
   odpovídalo celé mozaice. Podle potřeby ho dolaďte.
4. Uložte jako **`.fits`**, pokud chcete lineární data dál zpracovávat, nebo
   jako **`.tif`**, pokud chcete roztažený obrázek tak, jak ho vidíte.

---

## 5. Krok za krokem

**Zadávání čísel:** vedle každého posuvníku je číselné pole. Klikněte do něj,
napište hodnotu a stiskněte **Enter**. Hodnota se zaokrouhlí na krok a rozsah
posuvníku.

### 5.0 Předvolby

Nabídka **Předvolba** (levý sloupec, pod *RAW přes*) nastaví najednou všechny
hodnoty párování a skládání pro daný typ námětu. Každou hodnotu lze potom
dál měnit.

| Předvolba | Pro | Hlavní nastavení |
|---|---|---|
| **denní krajina** | denní panoramy, fotoaparát otáčený na místě | slabé odšumění, homografie, cylindrická projekce |
| **astro / noční krajina** | noční obloha, Mléčná dráha nad krajinou | silné odšumění, volnější párování, sférická projekce, +1 EV (pak **Automatická expozice**), vyšší práh rohů (méně falešných rohů v šumu) |
| **deep-sky mozaika** | mozaiky hvězdných polí z dalekohledu, např. FITS | málo odšumění (hvězdy jsou malé), model podobnosti (posun + rotace pole), bez zpřesnění, přímočará projekce; použijte **Automatické roztažení** |
| **makro / mikroskop** | preparát nebo stolek posouvaný pod pevným fotoaparátem | model posunu (snímky se jen posouvají), bez zpřesnění, přímočará projekce |
| **sken / plochý dokument** | mapy, obrazy, dokumenty fotografované po částech | afinní model, bez zpřesnění, přímočará projekce |

### 5.1 Snímky (Images)

| Prvek | Co dělá |
|---|---|
| **Přidat…** | Otevře dialog pro výběr souborů. Vybrat můžete více souborů (Ctrl/Shift + klik). |
| **Pole pro cestu** | Napište cestu k souboru nebo ke **složce** a stiskněte Enter. Složka načte všechny snímky v ní, seřazené podle jména. Soubory pojmenované `panorama…` (dřívější výsledky) se přeskočí. |
| **Seznam snímků** | Kliknutím snímek vyberete a zobrazíte. **Ctrl + klik** přidá snímek do výběru nebo ho z něj odebere, **Shift + klik** vybere rozsah a **Ctrl+A** (s myší nad seznamem) vybere vše. Seznam se posouvá kolečkem myši. |
| **Odebrat** | Odstraní vybrané snímky. Totéž udělá klávesa **Smazat**, když je myš nad seznamem. |
| **Vymazat vše** | Odstraní všechny snímky. |
| **▲ / ▼** | Posune vybrané snímky v seznamu nahoru nebo dolů. Na pořadí záleží jen v režimech párování *sequential* a *grid*. |
| **Reference** | Snímek, kolem kterého se ostatní umisťují; zůstane nezkreslený. **auto (střed)** zvolí snímek uprostřed skupiny, což obvykle dává nejvyrovnanější výsledek. |
| **RAW přes** | Který převodník RAW použít, viz [Soubory RAW](#8-soubory-raw). Platí pro snímky přidané poté. |
| **poloviční velikost** | Načte soubory RAW a soubory FITS z barevných kamer v poloviční velikosti: asi 4× rychleji a se čtvrtinou paměti. Platí pro snímky přidané poté. Při ukládání s **Výstupní velikost** nad 50 % se soubory RAW načtou znovu v plné velikosti. |

Značky v seznamu:

| Značka | Význam |
|---|---|
| modrý pruh | vybraný snímek; tmavší je aktuální snímek (pro zobrazení, jas a posouvání) |
| `R` | referenční snímek |
| `4345kp` | počet nalezených význačných bodů |
| `UNLINKED` | snímek se nepodařilo připojit k panoramě (viz [5.3](#53-krok-2-párování-matching)) |

Výsledky párování zůstanou zachované, když snímky přeřadíte nebo odstraníte
jiné snímky. Párovat znovu tedy nemusíte.

### 5.2 Krok 1: Význačné body (Features)

Význačné body jsou výrazná místa, například rohy, hrany a hvězdy, která lze
rozpoznat na dvou překrývajících se snímcích.

| Posuvník | Výchozí | Význam |
|---|---|---|
| **Velikost pro detekci (px)** | 2000 | Před hledáním se snímek zmenší na tuto velikost (delší strana). Větší je přesnější, ale pomalejší. |
| **Max. počet bodů** | 5000 | Horní mez na snímek. Body se rozmístí rovnoměrně po celém snímku. |
| **Práh rohů** | 0,001 | Minimální síla rohu. Nižší najde i slabší rohy (užitečné u oblohy nebo mlhy), vyšší jen výrazné. |
| **Odšumění (σ px)** | 1,5 | Před hledáním rozmaže šum snímače. Pro **noční snímky** a zašuměné snímky s vysokým ISO ho zvyšte (2–3); pro hvězdná pole a čisté denní snímky s jemnými detaily ho snižte (0,5–1). |

Klikněte na **Najít význačné body**. Náhled ukáže aktuální snímek s body jako
**žlutými kroužky**. Měly by být rozložené po částech snímku, které se
překrývají s jinými snímky. Znovu se zpracují jen snímky, u kterých se změnilo
nastavení. Tmavé a lineární snímky (noční záběry, FITS) nevadí: detekce si
kontrast upraví sama.

### 5.3 Krok 2: Párování (Matching)

Párování najde stejné body na dvou snímcích. **RANSAC** pak ponechá jen páry
bodů, které odpovídají jednomu geometrickému modelu (tzv. *inliery*).

**Které dvojice se zkoušejí (nabídka Pairs):**

| Pairs | Kdy použít |
|---|---|
| **všechny páry** | Výchozí. Každý snímek se zkouší s každým, takže **na pořadí nezáleží** a i více řad funguje samo. U mnoha snímků trvá déle (74 fotek ≈ 35 s). |
| **postupně** | Jedna řada, snímky v pořadí pořízení. Každý snímek se páruje jen s následujícím. Nejrychlejší. |
| **mřížka** | Více řad ve známém pořadí: **Sloupce mřížky** snímků v řadě, řada za řadou. Zkoušejí se sousedé vpravo, dole a úhlopříčně. Pokud byla každá druhá řada fotografována opačným směrem, zapněte **had**. |

**Nastavení:**

| Prvek | Výchozí | Význam |
|---|---|---|
| **Sloupce mřížky** | 5 | Snímků v řadě (jen v režimu grid). |
| **Poměrový test** | 0,8 | O kolik lepší musí být nejlepší shoda než druhá nejlepší. Nižší je přísnější (méně, ale spolehlivějších shod), vyšší dá více shod. |
| **Max. vzdálenost** | 0,35 | Jak odlišně mohou dva body vypadat, aby se ještě spárovaly. |
| **Tolerance RANSAC (px)** | 5,0 | Jak daleko (v pixelech) smí shoda ležet od modelu, aby se ještě počítala jako inlier. |
| **Min. inlierů** | 20 | Nejmenší počet inlierů, aby se dvojice počítala jako skutečný překryv. |
| **Model** | homography | Jak se jeden snímek zobrazí na druhý, viz níže. |
| **zpřesnit** | zapnuto | Globální zpřesnění, viz níže. |

**Kdy se dvojice počítá jako skutečný překryv:** potřebuje alespoň *Min.
inliers* inlierů, jasná většina jejích shod musí souhlasit a transformace musí
být věrohodná (nesmí snímek převrátit ani extrémně roztáhnout). Tím se
odfiltrují falešné shody mezi snímky, které se ve skutečnosti nepřekrývají.

**Modely:**

| Model | Kdy použít |
|---|---|
| **homografie** | Běžné panoramy: fotoaparát otáčený na místě. Výchozí. |
| **afinní** | Skeny nebo fotografie plochého předmětu z velké dálky. |
| **podobnost** | Jen posun, otočení a rovnoměrné zvětšení, např. mozaiky z dalekohledu s rotací pole. Stabilní i při malém počtu shod. |
| **posun** | Jen posun, např. skener nebo stolek mikroskopu. Nejrobustnější, nejméně pružný. |

**Umístění a globální zpřesnění (refine):** snímky se propojí přes své
nejsilnější dvojice, počínaje referenčním snímkem. Se zapnutým **zpřesnit** (jen
u modelu homography) se pak všechny dvojice řeší najednou jako jeden otáčející
se fotoaparát. Malé chyby se tak rozloží rovnoměrně, místo aby se sčítaly, a
uzavřou se smyčky mezi řadami; to odstraní většinu „duchů“. Zároveň se odhadne
soudkovité zkreslení objektivu (výrazné u širokoúhlých objektivů) a při
vykreslení se opraví. Dvojice, které do společného řešení nezapadají, se
**automaticky vyřadí**, protože jde obvykle o falešné shody nebo o něco, co se
pohnulo. Stavový řádek ukáže, kolik jich bylo vyřazeno, a **typickou chybu
dvojic** (1–3 px je dobré).

**Noční obloha: párování podle hvězd.** Rohové body jen zřídka spárují snímky,
na kterých je jen obloha: každá hvězda vypadá jako každá jiná. Když body
dvojice selžou, porovnají se proto její **hvězdy**: trojúhelníky sousedních
hvězd podle úhlů mezi nimi. Ty se nemění, když se otočí fotoaparát ani když se
otočí obloha, takže se spárují i snímky pořízené minuty po sobě. Takové dvojice
mají v menu **Pár** poznámku *(hvězdy)* a stavový řádek je spočítá. Přijmou se
jen dvojice s dostatkem hvězd v překryvu, z nichž se velká část shoduje.
U fotoaparátů mířících daleko od sebe (řada nahoru a řada dolů) leží vzdálené
rohy snímku za druhým fotoaparátem; tam se posuzuje jen část, kterou pokrývají
shody.

**Zoom, který se posunul.** Při míření nahoru nebo dolů se kroužek zoomu může
mezi snímky pootočit vlastní vahou. U souborů RAW je ohnisková vzdálenost
každého snímku známá: body se hledají ve všech snímcích ve stejném měřítku a
zpřesnění dopasuje každou ohniskovou vzdálenost zvlášť. Stavový řádek pak
napíše *Ohnisková vzdálenost se mezi snímky měnila (15–17 mm)*.

**Otáčející se obloha.** Obloha se otáčí o 15° za hodinu, takže řady pořízené
deset minut po sobě ji vidí posunutou o 2,5°. U souborů RAW (zaznamenávají čas
pořízení) zpřesnění změří toto otáčení z dvojic podle hvězd spolu s dvojicemi
na zemi, takže fotoaparáty jsou umístěné správně. Stavový řádek napíše *obloha
se mezi snímky otáčela o 14,9°/h*: blízko 15 znamená, že se to povedlo. Při
vykreslení se hvězdy všech snímků ukážou tak, jak byly v jednom okamžiku
(uprostřed focení), zatímco země a světlo oblohy zůstanou na svém místě; u
stromů a jiných siluet zůstane každý snímek, jak byl pořízen.

**Tlačítka:**

- **Spárovat**: spáruje dvojice zvolené v nabídce Pairs (paralelně), potom
  snímky umístí a zpřesní.
- **Znovu spárovat**: znovu spáruje jen zobrazenou dvojici s aktuálním
  nastavením.
- **Zahodit pár**: přestane používat zobrazenou dvojici (pro shodu, která je
  zjevně špatná); umístění se přepočítá bez ní.
- **Spustit všechny kroky**: body, párování, umístění a náhled najednou.

Pohled **Shody párů** ukazuje dva snímky vedle sebe:

- **zelené čáry**: inliery (správné shody)
- **červené čáry**: outliery. Skryjete je přepínačem **odlehlé body**.

Nabídka **Pár** vypisuje přijaté dvojice s počtem inlierů. Dvojice označené
*rejected* neprošly některou kontrolou; text pod nabídkou uvádí proč.

**Jak vypadá dobrý výsledek:** desítky zelených čar nebo více, všechny
popisující stejný posun, a průměrná chyba 1–2 px.

**Nepropojené snímky:** snímek bez přijaté dvojice k ostatním je v seznamu
označen `UNLINKED` a **do panoramy se nezahrne**, aby ji nepokazil. Zapněte
**ukázat nepropojené obrázky** (pravý sloupec), aby se takové snímky ukázaly v řadě
pod panoramou, a umístěte je ručně pomocí **Ctrl + tažení**.

### 5.4 Krok 3: Projekce (Projection)

Jak jednotlivé projekce vypadají, popisuje kapitola [Projekce](#7-projekce).

| Prvek | Význam |
|---|---|
| **Nabídka projekce** | 14 projekcí, viz [Projekce](#7-projekce) |
| **Ohnisko (× auto)** | Ohnisková vzdálenost se odhadne ze shod. Tento posuvník ji násobí. Vypadá-li panorama do stran stlačená nebo roztažená, zkuste hodnoty 0,8 až 1,2. |
| **Natočení (°)** | Otočí celou panoramu doleva nebo doprava: určuje, který směr bude uprostřed. Hodí se u 360° panoram (např. aby jádro Mléčné dráhy bylo uprostřed, ne na okraji). |
| **Sklon (°)** | Nakloní celou panoramu nahoru nebo dolů. Slouží k **narovnání prohnutého horizontu** (například když fotoaparát mířil dolů). |
| **Naklonění (°)** | Otočí celou panoramu. Slouží k vyrovnání šikmého horizontu. |
| *Jemné nastavení* | Yaw, pitch a roll mají posuvník, číselné pole a **◀ ▶**. Posuvníky pitch a roll jsou jemné u 0° (kde bývají opravy) a dál hrubší. Pomalým tažením číselného pole je to 0,02° na pixel, dvojklikem do něj můžete hodnotu napsat. **◀ ▶** krokují po 0,1° (se Shiftem po 1°). |
| **Vynulovat ohnisko / natočení / sklon / naklonění** | Vrátí tyto čtyři posuvníky na výchozí hodnoty. |

Titulek náhledu ukazuje velikost panoramy, ohniskovou vzdálenost v pixelech,
zorný úhel referenčního snímku a typickou chybu zarovnání.

Tyto prvky mění náhled okamžitě (jakmile náhled existuje).

### 5.5 Krok 4: Skládání (Compose)

| Prvek | Význam |
|---|---|
| **Prolnutí** | **chytré** (výchozí): vícepásmové prolínání s odstraněním duchů. Jemné detaily pocházejí v každém místě z jednoho snímku, takže nevzniknou dvojité obrazy lidí, světel ani blízkých předmětů. Rozdíly jasu se prolnou v širokém pásu, takže nevzniknou ostré spoje. Kde se překrývající snímky neshodují (něco se pohnulo), převezme se celá oblast z jednoho snímku. **prolnutí**: prosté plynulé míchání; kde se něco pohnulo, ukáže duchy. **šev**: každý pixel z nejvíce středového snímku; bez duchů, ale s ostrými spoji. **průměr**: prostý průměr; hlavně pro kontrolu zarovnání. |
| **Velikost náhledu (px)** | Velikost náhledu (delší strana). Větší je ostřejší, ale pomalejší. Při přiblížení se detail stejně dopočítá (viz [Pohyb v náhledu](#6-pohyb-v-náhledu)). |
| **Expozice (EV)** | Zesvětlí nebo ztmaví celou panoramu, od −3 do +8 EV (+1 EV = dvakrát jasnější). Tmavé astro panoramy často potřebují +1,5 až +4. |
| **Jas (výběr)** | Jasový faktor **vybraných** snímků. |
| **ukázat nepropojené obrázky** | Ukáže i snímky, které se nepodařilo propojit, v řadě pod panoramou, aby je šlo umístit ručně. |
| **Automatická expozice** | Nastaví **Expozice** podle aktuálního náhledu: typický jas na střední šedou, aniž by se přepálilo víc než nejjasnějších 0,5 % (hvězdy, lampy). |
| **Vyrovnat** | Spočítá jasové faktory snímků tak, aby si překryvy odpovídaly. |
| **Vynulovat posuny** | Zruší všechna ruční posunutí. |
| **Automatický ořez** | Ořízne panoramu na největší obdélník, který je celý pokrytý snímky, takže zmizí černé zubaté okraje. Náhled pak ukazuje oříznutý výsledek a **Uložit panorama…** ho uloží. Stavový řádek uvede, kolik panoramy zůstalo. Nejdřív narovnejte horizont (Pitch, Roll); z prohnuté panoramy zbude jen úzký pruh. |
| **Zrušit ořez** | Zruší ořez. Ořez se zruší i automaticky, když změníte cokoli, co snímky posune (projekci, ohnisko, yaw, pitch, roll, ruční posuny, párování). |
| **✂ Ořez** (nad náhledem) | Vyznačte na panoramatu část, která zůstane: natáhněte rámeček, jeho hrany a rohy ho upraví, tažením uvnitř ho posunete. Potvrďte **✂ Ořez: hotovo** (Esc zruší). Vykreslí a uloží se jen tato část, takže velké panorama potřebuje méně paměti a kvůli samotnému ořezu nemusíte do záložky Dodatečné úpravy. **Zrušit ořez** ho odstraní; stejně jako automatický ořez zmizí při změně geometrie. |
| **⬇ Uložit a upravit…** (nad náhledem) | Uloží panorama a pak ho otevře v záložce Dodatečné úpravy. |
| **Nudge ← ↑ ↓ →** | Posune aktuální snímek o jeden pixel náhledu (viz níže). |

V pohledu **Panorama** je obrys každého snímku vykreslen a očíslován:

- **oranžově**: aktuální snímek
- **světle modře**: další vybrané snímky
- **azurově**: referenční snímek
- **bíle**: ostatní snímky
- **přerušované žluté čáry**: vazby použité k umístění snímků

Přepínačem **překryvy** v liště je vypnete a uvidíte samotnou panoramu.

**Kliknutím do obrysu** snímek vyberete (funguje i s vypnutými překryvy).

**Ruční posouvání snímků:** držte **Ctrl** a táhněte snímek, nebo použijte
šipky či tlačítka **Posunout vybraný**. Snímek se posune spolu se snímky, které jsou k
němu připojené dál od reference (podél žlutých vazeb). Referenční snímek se
nehýbe; celou panoramu posunete pomocí Pitch a Roll.

### 5.6 Krok 5: Roztažení histogramu (Stretch)

Astronomické snímky (a tmavé převody RAW) jsou obvykle **lineární**: zajímavé
detaily leží v nejtmavších několika procentech rozsahu jasu a snímek vypadá
skoro černě. Roztažení histogramu je zviditelní.

| Prvek | Význam |
|---|---|
| **Histogram** | Rozložení jasu v náhledu panoramy (v pohledu Panorama) nebo v aktuálním snímku (v ostatních pohledech), v logaritmickém měřítku. **Modrá čára** značí černý bod, **bílá čára** bílý bod a **oranžová křivka** ukazuje, jak se vstupní jas (zleva doprava) převádí na výstupní (zdola nahoru). |
| **Černý bod** | Vše s tímto nebo nižším jasem bude černé. Nastavte ho těsně pod pozadí oblohy (levý okraj hlavního vrcholu histogramu). |
| **Středy** | Zesvětlí (menší hodnoty) nebo ztmaví (větší hodnoty) střední tóny bez ořezání. 0,5 je neutrální. Snímky hlubokého vesmíru obvykle potřebují 0,005–0,05. |
| **Bílý bod** | Vše s tímto nebo vyšším jasem bude bílé. |
| **Automatické roztažení** | Nastaví všechny tři hodnoty podle aktuálního histogramu: černý bod těsně pod pozadím oblohy, střední tóny tak, aby pozadí vyšlo asi na 25 % šedi (stejný princip jako automatické roztažení v PixInsightu). |
| **Vynulovat roztažení** | Zpět bez roztažení (0 / 0,5 / 1). |

- Roztažení mění jen to, jak se snímky **zobrazují** a jak se panorama ukládá
  do TIFF, PNG nebo JPEG. Párování pracuje vždy s původními daty.
- **Soubory FITS** se při načtení automaticky roztáhnou pro zobrazení, a to
  podle všech načtených snímků FITS dohromady.
- Změna roztažení je okamžitá, nic se nepřepočítává.
- **Expozice** se uplatní před roztažením.

**Zobrazovací režimy (jen pohled).** **Zobrazení** nad náhledem ukáže data jako
Siril: *Lineární*, *Logaritmus*, *Odmocnina*, *Druhá mocnina*, *Asinh*,
*AutoStretch* nebo *Vyrovnání histogramu*, s posuvníky **Low** a **High**:
hodnoty pod Low jsou černé, nad High bílé (pro slabá lineární data zkuste High
kolem 0,01–0,1; ↺ je vrátí). *Jak je zpracováno* ukazuje roztažení z tohoto
panelu. Zobrazovací režim nikdy nemění, co se uloží. Výsledek skládání hvězd
a záložka Dodatečné úpravy mají stejný ovládací prvek.

### 5.7 Zpět, znovu a projekty

**Zpět / znovu:** tlačítka **Zpět** a **Znovu** v liště, nebo **Ctrl+Z** a
**Ctrl+Y** (také **Ctrl+Shift+Z**). Stavový řádek uvede, co se vrátilo, např.
*„Undone: Pitch (°) 0 → 10“*.

- Vrátit lze: každý posuvník, nabídku a přepínač nastavení; přidání,
  odstranění a přeřazení snímků; výsledky párování (Match, Re-match, Discard);
  referenční snímek; ruční posuny; jas; automatický ořez.
- Tažení posuvníku je **jeden** krok, ať táhnete jakkoli dlouho. Změny dvou
  různých prvků jsou vždy dva kroky.
- Odstraněné snímky zůstávají kvůli vrácení v paměti, takže se vrátí okamžitě
  i s výsledky párování.
- Pamatuje se posledních 100 kroků. Vrátit nelze: věci týkající se jen
  zobrazení (zoom, zvolený pohled, překryvy), *RAW přes* a *half size* a
  ukládání souborů.

**Projekty:** **Uložit projekt…** (nahoře v levém sloupci) uloží soubor projektu
s příponou **`.panoproj`**. **Otevřít projekt…** ho znovu načte. Cestu k souboru
`.panoproj` můžete také napsat do pole pro cestu a stisknout Enter, nebo s ním
program spustit: `panostitch moje_pano.panoproj`.

Projekt obsahuje:

- snímky (úplnou cestu a cestu relativní k souboru projektu)
- všechna nastavení: posuvníky, nabídky, přepínače, včetně roztažení
- výsledky párování, takže se nic nemusí párovat znovu
- ruční posuny, jasové faktory, referenční snímek a ořez
- *RAW přes* a *half size*, aby se snímky načetly stejně

Samotné snímky v projektu **nejsou**; načítají se ze svých souborů. Přesunete-li
fotografie spolu se souborem projektu (např. celou složku), najdou se přes
relativní cestu. Chybějící soubory program ohlásí a vynechá.

Soubor je čitelný text (formát TOML). Jeho velikost tvoří hlavně uložené
shodné body: asi 50 kB na snímek.

---

## 6. Pohyb v náhledu

| Akce | Myš | Klávesnice | Tlačítko |
|---|---|---|---|
| Posun | tažení **levým** tlačítkem | šipky (pohledy Image a Pair) | |
| Přiblížení | kolečko nahoru | `+` | **Přiblížit +** |
| Oddálení | kolečko dolů | `-` | **Oddálit -** |
| Zpět na celek | | `F`, `0` nebo `Home` | **Přizpůsobit** |
| Výběr snímku (pohled Panorama) | klik do jeho obrysu | | |
| Zobrazit / skrýt obrysy, čísla, vazby | | | přepínač **překryvy** |
| Posun snímku (pohled Panorama) | **Ctrl + tažení** | šipky (`Shift` = 10 kroků) | **← ↑ ↓ →** |
| Zpět | | `Ctrl+Z` | **Zpět** |
| Znovu | | `Ctrl+Y` nebo `Ctrl+Shift+Z` | **Znovu** |

Tlačítka **Obrázek + body**, **Shody párů** a **Panorama** přepínají mezi
třemi pohledy.

**Další zkratky** (zkratku tlačítka ukáže najetí myší):

| Klávesy | Panorama stitcher | Star stacking |
|---|---|---|
| `Ctrl+Enter` | **Spustit všechny kroky** | **Start** |
| `P` / `Ctrl+P` | `P`: **Náhled panoramatu** | `Ctrl+P`: **Rychlý náhled** |
| `Ctrl+S` | uloží projekt (do otevřeného souboru projektu hned; poprvé se zeptá na název) | totéž |
| `Ctrl+Shift+S` | **Uložit panorama…** | |
| `Ctrl+O` | **Otevřít projekt…** | totéž |
| `Esc` | | zruší běžící skládání |
| `F` | přizpůsobí obraz náhledu | totéž |

Na macOS místo `Ctrl` použijte `Cmd`.

**Zavření okna** se nejdřív zeptá, pokud běží skládání nebo pokud práce v
některé kartě není uložená v projektu. Pak můžete projekt uložit, přesto
skončit, nebo zrušit.

**Ostré přiblížení:** náhled je zmenšený obrázek. Když panoramu přiblížíte a
chvíli počkáte, viditelná oblast se znovu vykreslí v rozlišení obrazovky z
původních snímků. Můžete tak zkontrolovat jemné detaily a spoje.

---

## 7. Projekce

Panorama zabírá víc, než dokáže ukázat plochá fotografie, takže ji program musí
*promítnout* do roviny. Každá projekce něco zkresluje:

| Projekce | Vypadá jako | Nejlepší pro | Omezení |
|---|---|---|---|
| **přímočará** | běžná fotografie: rovné čáry zůstanou rovné | úzké panoramy (do ~100°), architektura, mozaiky z dalekohledu | se šířkou záběru se okraje extrémně roztahují |
| **válcová** | vodorovné čáry se ohýbají, svislé zůstanou svislé | většina vodorovných panoram | ne pro vysoké záběry (horní a dolní okraj se roztahují) |
| **sférická** (sférická, ekvidistantní válcová) | jako válcová, ale méně roztažená svisle | velmi široké a 360° panoramy, prohlížeče panoram | rovné čáry se ohýbají |
| **Mercator** (Mercatorova) | mezi válcovou a sférickou | široké panoramy s určitou výškou | roztahuje se u horního a dolního okraje |
| **stereografická** | podobná rybímu oku, tvary zůstávají kulaté | velmi široké zorné úhly, kreativní záběry | silné zakřivení |
| **příčná válcová** | válcová položená na bok: svislé čáry se ohýbají, vodorovné zůstávají rovné | **svislé panoramy** (od horizontu k zenitu), vztyčený oblouk Mléčné dráhy | ne pro široké vodorovné záběry (přetečou přes horní a dolní okraj) |
| **příčný Mercator** (příčná Mercatorova) | jako příčná válcová, do stran méně roztažená | svislé panoramy s určitou šířkou | jako výše |
| **fisheye** (rybí oko, ekvidistantní) | kulatý obraz, vzdálenost od středu úměrná úhlu | **celooblohové pohledy**: Pitch nakloňte tak, aby byl zenit uprostřed | okraje stlačené |
| **fisheye ekvisolidní** (rybí oko, plochojevné) | kulatý obraz zachovávající poměr ploch | celooblohové pohledy, jak vypadá většina objektivů typu rybí oko | okraje stlačené ještě víc |
| **malá planeta** | země uprostřed, obloha kolem ní jako prstenec | 360° panoramy se zemí, kreativní | panorama musí sahat hodně dolů |
| **tunel** | obloha uprostřed, horizont kolem ní | 360° noční obloha, stopy hvězd, stromy zespodu | panorama musí sahat hodně nahoru |
| **ortografická** | zeměkoule viděná zvenku | kreativní; ukazuje polovinu koule | okraje jsou velmi stlačené |
| **Hammer** (Hammerova–Aitoffova) | oválná mapa celé koule se správným poměrem ploch | panoramy celé koule jako jeden obraz, celooblohové hvězdné mapy | tvary u okrajů zkreslené |
| **Mollweide** (Mollweideova) | oválná plochojevná mapa, u pólů plošší | jako Hammerova; klasická celooblohová mapa v astronomii | tvary u okrajů zkreslené |

Vypadá-li horizont jako úsměv nebo zamračení, upravte **Sklon**. Je-li šikmý,
upravte **Naklonění**. **Natočení** otáčí pohled do stran. U rybího oka, malé planety a
tunelu určuje Pitch, co bude uprostřed.

---

## 8. Soubory RAW

Podporováno: **Canon CR2 a CR3, Nikon NEF a NRW, Sony ARW a SRF, DNG, Fuji RAF,
Olympus ORF, Panasonic RW2, Pentax PEF, Samsung SRW** a další. Přesná podpora
fotoaparátů závisí na dekodéru; vestavěný zná většinu fotoaparátů posledních
dvou desetiletí, LibRaw (je-li nainstalované) asi 1250 fotoaparátů.

**RAW přes** volí dekodér:

| Volba | Dekodér | Charakter |
|---|---|---|
| **auto** | první, který uspěje, v pořadí níže; má-li snímek soubor `.pp3`, nejdřív RawTherapee | doporučeno |
| **vestavěný** | vlastní dekodér programu | vždy k dispozici, rychlý; neutrální převod s barvami a vyvážením bílé fotoaparátu, bez automatického zesvětlení, takže všechny snímky mají stejnou expozici |
| **libraw** | `dcraw_emu` (LibRaw) | podobný vestavěnému; pro fotoaparáty, které vestavěný dekodér nezná |
| **rawtherapee** | `rawtherapee-cli` | **použije vaše úpravy z RawTherapee** uložené v `snimek.NEF.pp3`; jinak výchozí vzhled RawTherapee |
| **darktable** | `darktable-cli` | výchozí převod darktable (včetně korekce objektivu, kde je dostupná); nejpomalejší |
| **dcraw** | `dcraw` | záloha; neumí CR3 |

Vždy je k dispozici jen built-in; ostatní se objeví, pokud je daný program
nainstalovaný v počítači. Skládání hvězd potřebuje lineární data, proto
používá jen built-in, libraw nebo dcraw.

Vnější dekodéry zapisují 16bitový obraz do dočasné složky, která se potom
smaže. Vedle vašich fotografií se nic nezapisuje.

**Tip:** pro jednotné barvy v celé panoramě použijte pro všechny snímky stejné
úpravy (stejný profil `.pp3`), nebo použijte **vestavěný** a barvy opravte až
nakonec.

---

## 9. Soubory FITS (astronomie)

Soubory s příponou **`.fits`, `.fit` nebo `.fts`** se čtou přímo, bez dalšího
softwaru. Podporováno:

| Obsah | Příklady | Jak se čte |
|---|---|---|
| **8-, 16-, 32- a 64bitová celá čísla, 32- a 64bitová desetinná čísla** | záznamy z kamer, výsledky skládání | převede se na 0…1 (16 bitů bez znaménka: 0…65535) |
| **Barevné (3 roviny)** | složené výsledky ze Sirilu, PixInsightu, DSS | jako RGB |
| **Černobílé** | černobílé kamery, jednotlivé filtry | jako šedá |
| **Surová data barevné kamery (Bayer)** | snímky ze ZWO, ToupTek, QHY… s hlavičkou `BAYERPAT` (RGGB, BGGR, GRBG, GBRG) | rozloží se na barvy (debayer); s **poloviční velikost** jeden barevný pixel na buňku 2×2 (rychlé, čisté), jinak interpolace v plné velikosti |
| **Pořadí řádků** | `ROWORDER` = `TOP-DOWN` nebo `BOTTOM-UP` (výchozí ve FITS) | snímek se zobrazí správně otočený |

Nepodporováno: **dlaždicově komprimované soubory (`.fz`)**. Nejdřív je
rozbalte, např. programem `funpack`. Další části souboru (tabulky, profily ICC)
se ignorují; použije se první obrázek.

**Poznámky k mozaikám hlubokého vesmíru:**

- Použijte předvolbu **deep-sky mozaika**. Dlaždice z dalekohledu se jen
  posouvají a otáčejí (rotace pole), což přesně popisuje model *similarity*.
- Nejlépe se spojují **složené a kalibrované dlaždice**. Jednotlivé krátké
  snímky mají málo viditelných hvězd a hodně šumu.
- Surová data barevné kamery nemají vyvážení bílé, takže vypadají zeleně;
  roztažení histogramu vyvážení barev nemění.
- Uložte jako **FITS**, pokud chcete pokračovat ve zpracování (odstranění
  gradientu, kalibrace barev, finální roztažení) v Sirilu nebo PixInsightu.
  Výstup FITS zachovává lineární hodnoty.

---

## 10. Ukládání

Klikněte na **Uložit panorama…** a zvolte název souboru. Typ souboru se
řídí příponou:

| Přípona | Výsledek |
|---|---|
| `.tif` / `.tiff` | 16 bitů na kanál, nejlepší pro další úpravy; uloží se **tak, jak to vidíte** (s expozicí a roztažením) |
| `.png` | 16 bitů na kanál; tak, jak to vidíte |
| `.jpg` | 8 bitů na kanál, menší soubor; tak, jak to vidíte |
| `.fits` / `.fit` / `.fts` | 32bitová desetinná čísla, tři barevné roviny; **lineárně** (expozice a jasové faktory se uplatní, ale **bez roztažení** a bez ořezu), pro astronomický software |

Jsou-li všechny načtené snímky FITS, navržený název souboru končí na `.fits`.

**Výstupní velikost** (nad tlačítkem pro uložení) určuje velikost uložené panoramy
v % plného rozlišení fotoaparátu; řádek pod ním ukazuje velikost v pixelech
(po náhledu). Tlačítka ¼, ½, full a 2× jsou rychlá volba. 100 % zachová
všechen detail; soubory RAW načtené s **poloviční velikost** se při ukládání načtou
znovu v plné velikosti, takže to trvá déle a potřebuje víc paměti. 50 % nebo
25 % dají rychle menší soubor, nad 100 % se zvětšuje (bez nového detailu).

Pod velikostí je odhad, kolik paměti uložení potřebuje a jak velký může být
soubor, vedle volného místa. Když se nevejde, zbarví se oranžově (⚠); po
najetí myší ukáže, co dělat. Uložení pak odmítne se stejnou radou, místo aby
zaplnilo paměť a celý počítač přestal reagovat. Načítání se při téměř plné
paměti zastaví dřív a ohlásí to.

Velká panorama, která se do paměti nevejde najednou, se vykreslí po pásech
řádků; stavový řádek pak pásy počítá. Celé se drží jen hotové panorama, a to
ve formátu souboru (3 bajty na pixel pro JPEG, 6 pro 16bitový TIFF a PNG), a
snímky znovu načtené v plné velikosti se drží v poloviční přesnosti. 16bitový
TIFF o 20 000 × 8 000 px tak potřebuje asi 1 GB plus jeden pás a snímky, kde
dřív potřeboval asi 8 GB. Při prolínání **Smart** se každý pás vykreslí s
přesahem, takže na sebe pásy navazují bez viditelného schodu. Uložení nikdy
nepoužije víc než 85 % paměti.

Panoramy širší nebo vyšší než 60 000 px se odmítnou. Téměř vždy to znamená, že
je některá dvojice spárovaná špatně nebo že projekce není vhodná; zkuste
cylindrical nebo spherical.

Během práce ukazuje stavový řádek uplynulý čas. Po uložení shrne vše krátký
**přehled** pod tlačítkem Save: kolik fotek je v panoramě (a kolik se
nespárovalo), čas spojování (načtení, párování a náhledy od přidání první
fotky), čas ukládání, velikost v pixelech a soubor s jeho velikostí, nebo
chyba, pokud se uložení nepovedlo. × ho zavře.

**Lineární, nebo roztažené.** Nad **Uložit panorama…** volba **Lineární (ke
zpracování)** uloží panorama bez roztažení (s expozicí a faktory jasu), pro
astronomický software, a **Roztažené (k prohlížení)** tak, jak je zobrazeno.
Dokud nevyberete, FITS a XISF jsou lineární a TIFF, PNG a JPEG roztažené.
Lineární data potřebují hodně úrovní: ukládejte je jako FITS, XISF nebo TIFF,
ne JPEG nebo PNG.

---

## 11. Řešení problémů

| Problém | Co zkusit |
|---|---|
| **Okno se neobjeví** | Na Windows se objeví zpráva s důvodem; na Linuxu a macOS spusťte program z terminálu a přečtěte si chybu tam. Okno potřebuje OpenGL 2.0 nebo novější: aktualizujte grafický ovladač (na starých PC ten od výrobce grafického čipu). |
| **Program se nečekaně zavřel** | Při dalším spuštění to oznámí stavový řádek. Podrobnosti jsou v `crash.log` ve složce nastavení (Linux `~/.config/panostitch`, Windows `%APPDATA%\panostitch`, macOS `~/Library/Application Support/panostitch`); přiložte ho prosím k hlášení problému. |
| **Ovládací prvky jsou na okraji useknuté** | Rozšiřte boční panel tažením jeho okraje. |
| **„Busy – wait…“** | Krok ještě běží; počkejte, až se stavový řádek změní. |
| **Snímek se při načítání přeskočí** | Stavový řádek uvede soubor a důvod. U RAW zkuste jiný převodník v **RAW přes**. Soubory FITS `.fz` nejdřív rozbalte. |
| **Panorama je skoro černá** | Fotografie: klikněte na **Automatická expozice** nebo zvyšte **Expozice (EV)** (noční záběry často potřebují +1,5 až +4), nebo snímky upravte v RawTherapee (jejich soubory `.pp3` se pak použijí). FITS a jiná lineární data: klikněte na **Automatické roztažení**. |
| **Snímky FITS jsou po načtení černé nebo bílé** | Automatické roztažení při načtení vychází ze všech snímků FITS; jeden silně pře- nebo podexponovaný snímek ho může rozhodit. Takové snímky odstraňte a klikněte na **Automatické roztažení** (nebo na **Vynulovat roztažení** a nastavte ho ručně). |
| **Snímky FITS jsou zelené** | Surová data barevných kamer nemají vyvážení bílé. Nejdřív je složte a barevně zkalibrujte, nebo barvy výsledku upravte v astronomickém softwaru. |
| **Málo žlutých bodů** | Snižte **Práh rohů**; zvyšte **Max. počet bodů**; zvyšte **Velikost pro detekci**. |
| **Body hlavně na trávě, obloze nebo šumu** | Zvyšte **Odšumění** na 2–3. |
| **Snímky označené UNLINKED** | Prohlédněte si je: opravdu se překrývají s jiným snímkem? Zvyšte **Poměrový test** (0,85–0,9) a **Max. vzdálenost** (0,4), snižte **Min. inlierů**, zvyšte **Odšumění** a znovu klikněte na **Spárovat**. Pokud to nepomůže, zapněte **ukázat nepropojené obrázky** a umístěte je pomocí **Ctrl + tažení**. |
| **Díra v zenitu nebo snímky jen s oblohou nepropojené** | Použijte soubory RAW (je potřeba jejich ohnisková vzdálenost a čas pořízení), nechte **všechny páry** a znovu **Spárovat**: snímky jen s oblohou se pak spárují podle hvězd. Každý snímek oblohy potřebuje pár desítek hvězd a dobrý překryv se sousedy. |
| **Řady mířící dolů (nebo nadir) nepropojené** | Země fotografovaná zblízka se posune při sebemenším pohybu fotoaparátu (paralaxa) a tráva v noci je hlavně šum. Foťte řady se zemí z panoramatické hlavy, která se otáčí kolem objektivu, nebo nadir dodatečně zakryjte. |
| **Mraky, voda nebo jiné pohyblivé věci** | Mezi snímky se mění, takže je nelze spolehlivě spárovat (snímky jen s mraky obvykle zůstanou nepropojené). Takové části foťte rychle po sobě, nebo je umístěte ručně. |
| **Použila se špatná dvojice** (čáry všemi směry, snímky na špatném místě) | Vyberte ji v nabídce **Pár** a klikněte na **Zahodit pár**. Se zapnutým **zpřesnit** se většina takových dvojic vyřadí automaticky. |
| **Duchové (zdvojené předměty)** | Nechte zapnuté **zpřesnit** a použijte **Blend → smart** (výchozí). Zbývající problémy pocházejí z věcí, které se mezi snímky změnily (např. svítilna rozsvícená jen na jednom snímku), nebo z **paralaxy**: blízké předměty se vůči pozadí posunou, když se fotoaparát neotáčí kolem objektivu (tzv. nodálního bodu). |
| **Viditelné jasové skoky** | **Vyrovnat**, potom jednotlivé snímky dolaďte pomocí **Jas (výběr)**. |
| **Prohnutý horizont** | Upravte **Sklon** (a **Naklonění**, je-li šikmý). |
| **Silně roztažený horní nebo dolní okraj** | Použijte projekci **sférická**; stavový řádek ji navrhne, když panorama sahá hodně nahoru nebo dolů. |
| **Panorama stlačená nebo roztažená** | Upravte **Ohnisko (× auto)**. |
| **Obrovská, zkreslená panorama** | Špatná dvojice (zkontrolujte dvojice), nebo použijte **sférická** / **válcová** místo rectilinear. |
| **Pomalé nebo dochází paměť** | Před načtením zapněte **poloviční velikost**; snižte **Velikost náhledu**. Snímky v plném rozlišení potřebují zhruba 12 bajtů na pixel, asi 300 MB na 24MP snímek. U mnoha snímků trvá párování *all pairs* déle; *grid* je rychlejší, když rozložení znáte. |
| **Auto crop ponechá jen úzký pruh** | Panorama je prohnutá. Nejdřív ji narovnejte pomocí **Sklon** a **Naklonění** (a zvolte vhodnou projekci), potom znovu **Automatický ořez**. |
| **Projekt se otevře bez některých snímků** | Stavový řádek vypíše chybějící soubory. Byly přesunuty nebo přejmenovány, a ne spolu se souborem projektu. Vraťte je zpět, nebo je přidejte znovu a spárujte. |
| **Zpět nic neudělá** | Zpět pracuje s ustálenými změnami; po poslední změně chvíli počkejte. Zatímco běží krok (*Busy*), čeká. |
| **Všechno je pomalé** | Před načtením zapněte **poloviční velikost** (RAW a FITS z barevných kamer); snižte **Velikost náhledu**. Na kartě skládání **Poloviční velikost** v RAW conversion. |

---

## 12. Tipy pro dobré panoramy

- **Překryv** sousedních snímků 30–50 %, i mezi řadami. U mozaik z dalekohledu
  obvykle stačí 15–25 %.
- **Otáčejte fotoaparátem** kolem objektivu, ne kolem sebe. Pokud to jde,
  použijte stativ, ideálně s panoramatickou hlavou.
- **Zamkněte expozici, vyvážení bílé a ostření** (ruční režim), aby si všechny
  snímky odpovídaly.
- **Přelepte kroužek zoomu** u nočních panoramat: při míření nahoru se zoom
  může posunout. PanoStitch si s tím poradí, ale pevná ohnisková vzdálenost se
  páruje nejlépe.
- Vyhněte se pohyblivým předmětům v překryvech, nebo je zachyťte jen na jednom
  snímku.
- **Držte fotoaparát vodorovně.** Nakloněný fotoaparát ohne horizont (oprava
  pomocí Pitch a Roll).
- U dlouhých sérií nejdřív zkontrolujte výsledek se zapnutým **poloviční velikost**.
- Pokud načítáte snímky po složkách, neukládejte výsledek do složky se
  zdrojovými snímky. Soubory pojmenované `panorama…` se přeskakují, jiné názvy
  ale ne.

---

## 13. Skládání hvězd

Druhá karta, **Skládání hvězd (krajina / stopy)**, spojí mnoho snímků téže
scény do jednoho obrázku: oblohy s nízkým šumem a ostrými hvězdami, nebo
hvězdných stop. Je určená pro krajinářskou astrofotografii: fotoaparát stojí
na stativu, obloha se mezi snímky pohybuje, popředí ne.

Funguje jako Sequator, ale s tím, co Sequatoru chybí: bias a flat-dark snímky,
32bitový lineární výstup, samostatný snímek popředí, hvězdné stopy bez mezer,
automatické rozpoznání oblohy a žádná černá skříňka: každý snímek se změří a
ohodnotí, graf ukáže naměřené hodnoty a protokol vypíše každý krok, co udělal a
jak dlouho trval.

```
┌──────────────────────┬──────────────────────────────────────┬─────────────────────┐
│ seznam vstupů        │ [Base image] [Result] [Frame report] │ Post-processing     │
│  Star images, Base,  │ [Fit] [Open project…] [Save project…]│  Background extr.   │
│  kalibrace, Fore-    │                                      │  Color calibration  │
│  ground, Output      │                                      │                     │
│ Add Remove Clear     │   základní snímek (zde se kreslí     │  Colour noise       │
│ Use as base          │   obloha), výsledek nebo report      │  Light pollution    │
│ seznam nastavení     │                                      │  Brightness, Tone   │
│ volby zvýrazněné     │                                      │  Color space        │
│ položky              │                                      │  [Save result…]     │
│                      │                                      │  [Use in Stitcher]  │
│ [Quick prev.] [Start]│  stavový řádek                       │                     │
└──────────────────────┴──────────────────────────────────────┴─────────────────────┘
```

**Klikněte na položku** v seznamu vstupů nebo v seznamu nastavení: pod
seznamy se objeví její volby a vysvětlení.

**Předvolby** (nad seznamem nastavení) nastaví vše pro typický námět jedním
kliknutím: **Krajina s Mléčnou dráhou** (zmrazené popředí, oblast oblohy
detekovaná automaticky, korekce zkreslení, světelné znečištění), **Star
trails** (vyplněné mezery) a **Deep sky (telescope)** (celý snímek je obloha,
extrakce pozadí, kalibrace barev, automatické roztažení). Každé nastavení lze
dál měnit. Během skládání ukazuje ukazatel průběhu pod náhledem krok, uplynulý
čas a odhadovaný zbývající čas. Po skončení se nahoře v pravém panelu objeví
krátký **přehled**: kolik snímků se složilo z kolika (včetně vynechaných a
vyřazených), kalibrační snímky, jak dlouho to trvalo, velikost výsledku a
uložený soubor s jeho velikostí, nebo co se pokazilo. × ho zavře.

### 13.1 Rychlý start

1. Zvýrazněte **Snímky oblohy**, klikněte na **Přidat…** a vyberte snímky oblohy
   (nebo zadejte cestu k souboru či složce do pole pro cestu a stiskněte Enter).
2. Zvýrazněte **Oblast oblohy**, zvolte **Hraniční čára** a v náhledu základního
   snímku táhněte čáru podél horizontu, nebo klikněte na **Rozpoznat oblohu** a
   masku opravte štětcem. Hvězdy se hledají jen na obloze.
3. Klikněte na **Rychlý náhled**: malý, rychlý výsledek z nejvýše 10 snímků
   pro kontrolu nastavení. Nic se neukládá.
4. Zkontrolujte řádek **Uložit jako** pod seznamem vstupů: napište název souboru
   nebo vyberte navržený název v rozbalovacím seznamu vedle, ve druhém
   seznamu zvolte typ souboru a tlačítkem **Složka…** složku (výchozí:
   `stacked.tif` vedle prvního snímku oblohy). Pokud soubor už existuje, řádek
   to oznámí; **další volný název** přidá číslo. Pod ním je odhad, kolik místa
   snímky při skládání potřebují (podle počtu a velikosti) a zda zůstanou v
   paměti, nebo na disku; když místo nestačí, zbarví se oranžově (⚠). Potom
   klikněte na **Start**.
5. Upravte dodatečné zpracování vpravo; výsledek se hned aktualizuje.
   **Uložit výsledek…** ho uloží znovu s novým nastavením.

### 13.2 Vstupní soubory

| Položka | Co nafotit |
|---|---|
| **Snímky oblohy** (*Snímky hvězd* v jednoduchém režimu) | Snímky oblohy: stejná ohnisková vzdálenost, clona a expozice. Nejlepší je RAW; TIFF, FITS nebo XISF jen pokud jsou data stále lineární (neupravená). **Sjednotit expozici** vyrovná snímky pořízené za různých podmínek. |
| **Základní snímek** (*Základní snímek* v jednoduchém režimu) | Snímek, ke kterému se ostatní zarovnají. Prázdné = prostřední světlý snímek. Jiný zvolíte v rozbalovacím seznamu vedle **Základní snímek**, nebo vyberte snímek v seznamu světlých snímků a klikněte na **Použít jako základní**. |
| **Tmavé snímky** (*Tmavé snímky* v jednoduchém režimu) | Tmavé snímky (darky): krytka na objektivu, stejné ISO a čas, pořízené těsně před snímky oblohy nebo po nich. Odstraní tepelný šum a horké pixely. |
| **Bias snímky** (*Bias snímky* v jednoduchém režimu) | Nejkratší čas, krytka na objektivu, stejné ISO. |
| **Flaty** (*Snímky vinětace* v jednoduchém režimu) | Flaty: rovnoměrně osvětlená bílá plocha, stejná ohnisková vzdálenost a clona. Opraví tmavší rohy a stíny prachu. |
| **Flat darky** (*Flat darky* v jednoduchém režimu) | Darky k flatům: stejné ISO a čas jako vinětační snímky. |
| **Snímek popředí** | Nepovinné: jeden snímek země ze stejného místa stativu – za modré hodinky, s dlouhou expozicí nebo nízkým ISO. Pod oblastí oblohy nahradí (zašuměné, tmavé) složené popředí. **Přizpůsobit jas** ho přizpůsobí jasu složeného popředí; **Expozice (EV)** ho odtud zesvětlí nebo ztmaví. Potřebuje oblast oblohy. |
| **Výstup** | Nastavuje se v řádku **Uložit jako**: název, typ a složka. `.tif` / `.png` (16 bitů), `.jpg` (8 bitů) nebo `.fits` / `.xisf` (32bitová čísla s plovoucí čárkou, lineární, k dalšímu zpracování v Siril, PixInsight apod.). |

Všechny kalibrační snímky jsou nepovinné. Při vysokém ISO hodně pomůže i
několik tmavých snímků.

**Seznamy souborů:** každá skupina má své soubory v rozbalovacím seznamu
vedle svého názvu, takže ani stovky světlých snímků není třeba procházet.
Vyberte v něm soubor a uvidíte ho v náhledu; **Odebrat** a **Použít jako základní**
pracují s tímto souborem, **Vymazat** vyprázdní celou skupinu.

**Zašuměné flaty** (málo snímků, nebo příliš tmavé v jedné barvě, např. při
modrém zdroji světla) se automaticky vyhladí; záznam postupu to uvede i s
tím, jak byly zašuměné. Jinak by se jejich šum otiskl do každého snímku a při
posunu hvězd by z něj vznikly jemné pruhy.

**Dočasné soubory:** mezivýsledky se na disk neukládají. Snímky se drží v
paměti, nebo – zabírají-li víc než polovinu volné paměti – v jednom dočasném
souboru ve složce výstupu, který se smaže hned po skončení (za běhu není ani
vidět). Záznam postupu uvede, co platí. Pokud na něj není místo, běh se
ještě před začátkem zastaví se zprávou: zapněte **Poloviční velikost** (čtvrtina
místa) nebo ukládejte výstup na disk s víc volného místa. Ani mastery (dark,
bias, flat) se neukládají.

### 13.3 Nastavení

| Nastavení | Co dělá |
|---|---|
| **Kompozice** | **Zarovnat hvězdy**: snímky se zarovnají podle hvězd a spojí. **Akumulace** je průměruje; **Vybrat nejlepší pixely** (výchozí) vynechá odlehlé hodnoty (letadla, satelity); **Zmrazit zemi** zarovná jen oblohu, takže obloha i popředí zůstanou ostré (potřebuje oblast oblohy); **Jen zarovnat** jen zapíše zarovnané snímky. **Hvězdné stopy**: bez zarovnání, obloha si ponechá nejjasnější hodnotu každého pixelu. **Stmívání stop**: **Náběh** (stopy začínají slabě, efekt pohybu ze Sequatoru), **Doběh** nebo **Oba konce** (zúžení na obou koncích jako u komety). **Vyplnit mezery mezi snímky**: krátké přestávky intervalu fotoaparátu se vyplní podle změřeného pohybu hvězd, takže stopy jsou souvislé čáry, ne čárkované. |
| **Oblast oblohy** | Kde je obloha. **Celá obloha**: celý snímek je obloha (bez popředí, např. snímky dalekohledem). **Hraniční čára**: táhněte čáru podél horizontu. **Přechod**: čára s plynulým přechodem. **Nepravidelná maska**: oblohu malujte levým tlačítkem myši, mažte pravým; **Velikost štětce** (posuvník nebo kolečko myši). **Rozpoznat oblohu** namaluje první masku podle jasu a hvězd – přibližně, proto ji zkontrolujte a opravte štětcem. **Rozmazání masky** změkčí okraj masky, jen směrem do oblohy, takže popředí nedostane nic z oblohy (žádné svatozáře podél horizontu). **Ukázat nalezené hvězdy** zakroužkuje hvězdy, které poslední běh našel v zobrazeném snímku. |
| **Sigma clipping** | Jak přísně se odstraňují odlehlé hodnoty (dolní / horní × σ). Obvykle 2–3; nižší odstraní víc, ale nechá víc šumu. Potřebuje aspoň 4 snímky, nejlépe 10 a více. |
| **Vyřazování snímků** | Vynechá snímky zřetelně horší než typický: mraky nebo rosa (slabší hvězdy), málo hvězd, rozmazané nebo protáhlé hvězdy (zaostření, vítr, ťuknutí do stativu). |
| **Převod RAW** | Vyvážení bílé (fotoaparát, průměr snímku, denní světlo), demozaikování (výchozí AHD; DHT nebo AAHD mají při vysokém ISO často méně barevného šumu) a **poloviční velikost** (4× méně pixelů, mnohem rychlejší). |
| **Omezit dynamický šum** | Najde a odstraní horké pixely i bez tmavých snímků. |
| **Omezit zkreslení** | Opraví zkreslení objektivu, které v rozích dlouhých sérií zanechává krátké stopy: **Objektiv**, **Složitá** (ultraširokoúhlé objektivy, refrakce u horizontu) nebo **Automaticky**. |
| **Sloučit 4 pixely** | Sečte 2×2 pixely: jasnější a méně zašuměné, poloviční šířka i výška. |
| **Timelapse** | Složí vždy N po sobě jdoucích snímků do jednoho; vznikne očíslovaná řada snímků s nízkým šumem ve složce vedle výstupu. |

### 13.4 Zobrazení

- **Základní snímek**: snímek, ke kterému se ostatní zarovnávají; zde se kreslí
  oblast oblohy. Kliknutím na soubor v seznamu vstupů ho zobrazíte.
- **Výsledek**: výsledek po dodatečném zpracování. Tažením posouváte, kolečkem
  myši zvětšujete, **Přizpůsobit** ukáže celý.
- **Report snímků**: graf všech měření pro každý snímek, vždy vydělených
  mediánem, takže špatný snímek vyčnívá (červeně = vynechaný, modře =
  základní snímek); pod ním řádek pro každý snímek oblohy: nalezené hvězdy,
  velikost hvězd (FWHM), protažení, úroveň oblohy, světlo (jas hvězd proti
  základnímu snímku), chyba zarovnání, zda byl snímek použit a proč ne. Pod
  tabulkou je **protokol zpracování**: každý krok běhu, co udělal (kalibrace,
  oblast oblohy, použité snímky, zkreslení, vyplnění mezer, popředí, dodatečné
  zpracování) a jak dlouho trval. **Kliknutím na snímek** v grafu nebo v
  tabulce ho vyřadíte (nebo znovu zařadíte); potom znovu klikněte na **Start**.

### 13.5 Dodatečné zpracování

Použije se na výsledek hned; **Start** ukládá s tímto nastavením, **Save
result…** uloží znovu.

| Nastavení | Co dělá |
|---|---|
| **Extrakce pozadí** | Odečte hladký model pozadí oblohy (přechody od světelného znečištění, Měsíce, vinětace); obloha si ponechá průměrnou úroveň. Pro **Celou oblohu** (deep sky) může být **Model** **RBF**: pružná plocha proložená vzorkovacími čtverci oblohy, která zvládne několik září nebo jasný roh, na což polynom nestačí; **Vzorků na řádek** (čtverce podél delší strany) a **Vyhlazení** určují, jak těsně plocha sleduje pozadí; **Tolerance mřížky** určuje, jak moc smí čtverec vyčnívat, než se počítá za objekt (vyšší ponechá víc), a **Zobrazit vzorky** je vyznačí na výsledku (zelená použito, červená vynecháno). Hvězdy, galaxie, mlhoviny a Mléčná dráha i okolí kolem nich se ze vzorků vynechají samy. Krajinky (ostatní oblasti oblohy) používají **Polynom** zvoleného **Stupně**. |
| **Kalibrace barev** | **Auto**: pozadí oblohy bude neutrálně šedé a průměrná hvězda bílá. **Ruční**: na výsledku natáhněte **▭ Oblast pozadí** přes prázdnou oblohu a **▭ Bílá reference** přes něco, co má být v průměru bílé (pole hvězd, galaxie, hvězda podobná Slunci); **Teplota** a **Odstín** doladí navíc. Bez oblastí platí jen teplota a odstín. |
| **Snížit barevný šum** | Odstraní barevné tečky a skvrny: vlnková redukce šumu jen na barevných složkách, jas zůstane přesně jako byl. |
| **Světelné znečištění** | **Deep sky**: odstraní hladkou záři. **Nerovnoměrné**: sleduje záři po dlaždicích (kopule světla měst na širokoúhlých snímcích). **Síla** určuje, kolik se odstraní. Nejdřív opravte vinětaci (vinětační snímky), jinak se zhorší. |
| **Zesílit světlo hvězd** | Zvýrazní hvězdy nad pozadím oblohy, aniž by zesílilo jeho šum. |
| **Automatický jas** | Nastaví oblohu na příjemný jas, aniž by se přepálily nejjasnější hvězdy. |
| **Vysoký dynamický rozsah** | Jasné hvězdy a světla stlačí místo přepálení. |
| **Tón** | **křivka sRGB** (jako fotoaparát), **MTF roztažení** (černý bod, střední tóny, bílý bod; automaticky nebo ručně), **Arcsinh roztažení** (faktor roztažení; zachová barvy hvězd), **GHS roztažení** (generalizované hyperbolické: síla, lokální intenzita, bod symetrie) nebo **Lineární** (bez křivky). U Arcsinh a GHS se černý bod (a u GHS bod symetrie) měří na pozadí oblohy, pokud neodškrtnete **Černý bod z pozadí**. |
| **Barevný prostor** | sRGB nebo Adobe RGB pro výstup TIFF a JPEG. |
| **Uložit: Lineární / Roztažené** | **Lineární (ke zpracování)**: do souboru jde světlo tak, jak bylo naměřeno (po extrakci pozadí, kalibraci barev a jasu), pro záložku Dodatečné úpravy, Siril nebo PixInsight; v běžných prohlížečích vypadá tmavě, proto FITS, XISF nebo TIFF. **Roztažené (k prohlížení)**: do souboru jde obrázek tak, jak je zobrazen, připravený k prohlížení a sdílení. Dokud nevyberete, FITS / XISF dostanou lineární data a TIFF / PNG / JPEG roztažený obrázek. |
| **Použít ve skládání panoramat** | Přidá uložený výsledek do karty Panorama stitcher jako jeden díl panoramatu (viz 13.7). |

### 13.6 Projekty

**Uložit projekt…** zapíše soubor `.stackproj` se seznamy souborů, oblastí
oblohy a všemi nastaveními. **Otevřít projekt…** (nebo `panostitch
noc.stackproj`) ho znovu otevře. Soubory se hledají podle plné cesty, nebo
relativně k souboru projektu, pokud byla složka přesunuta.

### 13.7 Panoramata ze složených dílů

Panorama Mléčné dráhy s nízkým šumem: na každé pozici panoramatu pořiďte
několik snímků (pozice se překrývají), potom:

1. Složte snímky první pozice (Start) a klikněte na **Použít ve skládání panoramat**.
2. Nahraďte snímky oblohy snímky další pozice, dejte výstupu nový název,
   Start, znovu **Použít ve skládání panoramat**; a tak dále.
3. Na kartě Panorama stitcher spojí **Spustit všechny kroky** složené díly.

Každý díl ukládejte jako `.tif`; stejné dodatečné zpracování všech dílů
udrží jejich jas a barvy stejné.

### 13.8 Tipy

- Pořiďte aspoň 10–20 snímků oblohy se stejným nastavením a hned potom
  10–20 tmavých snímků, aniž byste pohnuli fotoaparátem.
- Oblast oblohy nastavte pečlivě: stromy a budovy započtené do oblohy zarovnání
  rozmaže. **Rozpoznat oblohu** je rychlý začátek, ne poslední slovo.
- Po Quick preview se podívejte do Frame report: graf hned ukáže, které
  snímky jsou horší (mraky, vítr, rosa).
- Když záleží na popředí, použijte **Zmrazit zemi**; když ne, **Select best
  pixels**.
- Pro zpracování v jiných programech ukládejte do `.fits` a zapněte **Linear
  (for processing)**.
- Z terminálu skládá bez okna `panostack`; `panostack --help` vypíše jeho
  volby (viz [Použití bez okna](#14-použití-bez-okna)).

---

## 14. Použití bez okna

**Skládání hvězd z terminálu:** `panostack` skládá stejně jako karta skládání
hvězd. `panostack --help` vypíše všechny volby. Příklady:

```
panostack --mode freeze --horizon 0.62 --darks darks/ -o noc.tif svetla/
panostack --mode trails --fade in --fill-gaps --auto-sky -o stopy.jpg svetla/
panostack --preview svetla/            # rychlá kontrola, nic se neukládá
```

Na konci se vypíše přehled snímků a záznam postupu.

**Spojování z vlastního programu v Rustu:** přidejte PanoStitch jako závislost
(`panostitch = { path = "…/PanoStitch" }` v souboru `Cargo.toml`), potom:

```rust
use panostitch::io;
use panostitch::stitcher::{self, Projection, StitchParams};

fn main() -> Result<(), String> {
    let paths = vec!["a.NEF".to_string(), "b.NEF".into(), "c.NEF".into()];
    let p = StitchParams { projection: Projection::Spherical, ..Default::default() };
    let pano = stitcher::stitch_files(&paths, &p, true, &mut |m| println!("{m}"))?;
    io::save_panorama("pano.tif", &pano)
}
```

Úplný příklad je v `examples/scripted.rs`
(`cargo run --release --example scripted -- a.NEF b.NEF c.NEF`).
`stitch_files(cesty, parametry, raw_half, log)` snímky načte a spojí;
`stitch(snímky, parametry, log)` spojí snímky, které už jsou v paměti.
Položky `StitchParams` a jejich výchozí hodnoty:

| Položka | Výchozí | Význam |
|---|---|---|
| `layout` | Layout::All | které dvojice párovat: `All`, `Sequential`, `Grid` |
| `cols`, `snake` | 0 (all images), `false` | rozložení do mřížky |
| `refine` | `true` | globální zpřesnění |
| `feature_scale` | `0.5` | měřítko hledání bodů |
| `nkeypoints` | `5000` |  |
| `denoise` | `1.5` |  |
| `ratio` | `0.8` |  |
| `max_distance` | `0.35` |  |
| `model` | `Model::Homography` | `Homography`, `Affine`, `Similarity`, `Translation` |
| `ransac_thresh` | `5.0` |  |
| `min_inliers` | `20` |  |
| `projection` | `Projection::Cylindrical` | libovolná z kapitoly [Projekce](#7-projekce), např. `Fisheye`, `LittlePlanet`, `Mollweide` |
| `blend` | `Blend::Smart` | `Smart`, `Feather`, `Seam`, `Average` |
| `sharpness` | `3.0` | režim feather: vyšší = užší přechody |

**Mozaika FITS s roztaženým TIFF a lineárním výsledkem FITS:**

```rust
use panostitch::stitcher::{Model, Projection, StitchParams};

let p = StitchParams { model: Model::Similarity, refine: false,
                       projection: Projection::Rectilinear, denoise: 0.75, ..Default::default() };
let mosaic = stitcher::stitch_files(&tiles, &p, false, &mut |_| {})?;
io::save_panorama("mosaic.fits", &mosaic)?;                          // linear
let (black, mid, white) = stitcher::auto_stretch(&mosaic, 0.25, 2.8);
io::save_panorama("mosaic.tif", &stitcher::hist_stretch(&mosaic, black, mid, white))?;
```

Skládání hvězd z kódu: vyplňte `stacker::StackConfig` (začněte od
`StackConfig::default()`) a zavolejte `stacker::stack(&config, false, &progress, None)`.

---

## 15. Soubory

| Soubor | Účel |
|---|---|
| `panostitch` (`panostitch.exe`) | Program (okno). |
| `panostack` (`panostack.exe`) | Skládání hvězd z příkazové řádky (`panostack --help`). |
| `*.panoproj`, `*.stackproj`, `*.tlproj` | Vaše soubory projektů (kamkoli je uložíte). |
| `ui.toml` ve složce nastavení | Přepínače vzhledu (viz [Spuštění programu](#2-spuštění-programu)). |
| `remembered/`, `recent_projects.txt`, `last_folder.txt` ve složce nastavení | Nastavení pro příští spuštění, nedávné projekty, složka, ve které začínají dialogy. Smazáním začnete načisto. |
| `docs/manual_cs.md` | Tato příručka (česky). |
| `docs/manual_en.md`, `docs/manual_de.md`, `docs/manual_es.md`, `docs/manual_fr.md`, `docs/manual_pl.md` | Příručka v angličtině, němčině, španělštině, francouzštině a polštině. |

Zdrojový kód (pro vlastní sestavení programu):

| Cesta | Obsah |
|---|---|
| `Cargo.toml` | Projekt a knihovny, které používá. |
| `src/image.rs`, `src/io/` | Obrazy; načítání a ukládání (JPEG, PNG, TIFF, BMP, RAW, FITS, XISF). |
| `src/stitcher/` | Spojování: body, párování, umístění, zpřesnění, projekce, prolínání. |
| `src/stacker/` | Skládání hvězd. |
| `src/finish.rs` | Dodatečné úpravy: geometrie, roztažení (MTF, arcsinh, GHS), vyvážení bílé, světla, křivky, barvy, detail. |
| `src/gui/` | Okno: čtyři karty, dialogy souborů, vestavěná příručka a texty okna v šesti jazycích (`i18n_*.rs`). |
| `src/bin/` | Oba programy, `panostitch` a `panostack`. |
| `tests/` | Testy: skládání na umělých nočních scénách, spojování, dodatečné úpravy, formáty souborů a karty okna. Spustíte je příkazem `cargo test --release`. |
| `examples/scripted.rs` | Použití spojování z vlastního kódu v Rustu. |

---

## 16. Dodatečné úpravy

*Novinka ve verzi 0.4 (ve vývoji).* Karta **Dodatečné úpravy** provede
poslední kroky se složeným nebo spojeným snímkem, takže na ně nepotřebujete
jiný program: otočení a ořez, roztažení lineárních dat, vyvážení bílé, světla,
křivky, barvy, šum, hvězdy, doostření a uložení v požadované velikosti.

**Jak dostat snímek dovnitř:** **Otevřít…** (Ctrl+O) nebo napište cestu a
stiskněte Enter; tlačítko **✎ Použít v Dodatečných úpravách** na kartě skládání
hvězd; nebo **✎ Dodatečně upravit** ve spojování po uložení panoramy. Soubory FITS
a XISF se berou jako lineární data a začínají automatickým roztažením; JPEG,
TIFF a PNG jako hotové snímky (bez roztažení). Ručně to přepíná **Linear data
(needs stretching)**.

Náhled pracuje s kopií o nejvýše 2000 px, takže na posuvníky reaguje téměř
hned. **Uložit** zpracuje celý snímek na pozadí přesně se stejným nastavením;
hodnoty, které program měří (automatické roztažení, neutrální pozadí), se
převezmou z náhledu, takže uložený soubor vypadá jako náhled.

Kroky běží vždy v tomto pořadí, vlevo shora dolů:

| Oddíl | Co dělá |
|---|---|
| **Otočit a oříznout** | ⟲ ⟳ otočení o 90°, **Zrcadlit** (zrcadlení), **Otočit** o libovolný úhel (jemné u 0°, jako Yaw/Pitch/Roll ve spojování), **Odříznout prázdné rohy** (odříznout prázdné rohy po otočení), ořez z každé strany nebo na tvar (3:2, 4:3, 16:9, 4:5, 1:1, 2:1). |
| **Roztažení** (lineární data) | Nejdřív **Extrakce pozadí** a **Kalibrace barev** (obojí volitelné, na lineárních datech), potom metoda roztažení (viz níže). |
| **Vyvážení bílé** | Na lineárním světle, před roztažením. **◎ Vzorek šedé**, potom táhněte obdélník přes něco, co má být neutrální (pozadí oblohy, mrak): podle něj se nastaví násobitele barev. **Neutrální pozadí** udělá tmavší polovinu šedou. **Teplota** a **Odstín** doladí. Hotové snímky se kvůli tomu vyjmou z křivky sRGB a pak do ní vrátí, takže vyvážení sedí v každém jasu. |
| **Světlo** | **Expozice** (EV), **Kontrast**, **Světla** (kolem 75 %), **Středy** (50 %), **Stíny** (25 %), **Bílé** a **Černé** (krajní body), jako ve fotoeditorech. |
| **Křivky** | Jedna křivka pro všechny barvy (**RGB**) a po jedné pro **Červená**, **Zelená**, **Modrá**. Kliknutím přidáte bod, tažením ho posunete, pravým tlačítkem odeberete. Histogram za křivkou je histogram toho, co do ní vstupuje. • označuje změněné křivky. |
| **Barvy** | **Sytost** (0 = černobílé) a **Živost** (mdlé barvy víc než sytré). |
| **Detaily** | **Redukce šumu** (vyhladí plochá místa, zachová hrany a hvězdy), **Barevný šum**, **Zmenšení hvězd** (menší hvězdy), **Doostřit** a jeho poloměr. Poloměry jsou v pixelech celého snímku, takže náhled odpovídá uloženému výsledku. |
| **Obloha a popředí** | Vlastní vzhled pro oblohu a pro popředí, navíc ke všemu výše, přes masku (viz níže). |

**Metody roztažení:**

| Metoda | Použití |
|---|---|
| **Vypnuto** | Snímek je už hotový. |
| **křivka sRGB** | Jako fotoaparát: pro lineární data, kterým stačí běžná gama. |
| **MTF, automaticky** | Černý bod těsně pod pozadím oblohy, střední tóny tak, aby pozadí mělo jas **Pozadí** (jako AutoStretch v Sirilu / STF v PixInsightu). **Každá barva zvlášť** (výchozí) měří červenou, zelenou a modrou zvlášť, takže barevný nádech složeného snímku obraz neschová; data se přitom nemění. **Automaticky → ručně** pokračuje ručně od naměřených hodnot. |
| **MTF, ručně** | Černý bod, střední tóny a bílý bod (midtones transfer function). |
| **Arcsinh** | Inverzní hyperbolický sinus: silné roztažení, které zachová barvy hvězd. **Faktor roztažení** 1–10000; hluboké snímky se slabým pozadím potřebují několik tisíc. |
| **Zobecněná hyperbolická (GHS)** | **Roztažení ln(D+1)** (síla), **Lokální intenzita b** (jak úzce se roztažení soustředí kolem bodu symetrie), **Bod symetrie** (kde je roztažení nejsilnější: dejte ho na pozadí oblohy), **Chránit stíny** a **Chránit světla** (pod / nad nimi přímka). |

U Arcsinh a GHS: **Inverzní** použije inverzní křivku (stlačuje místo
roztahování, např. když je roztažení příliš silné); **Zachovat barvy hvězd**
roztahuje jas a zachová barvu každého pixelu (vypnuto: každá barva zvlášť,
sytější obloha, bledší hvězdy); **Body z pozadí** nastaví černý
bod těsně pod pozadí nejtmavší barvy a u GHS bod symetrie na pozadí.

**Zobrazení** (nad náhledem) mění jen zobrazení náhledu, nikdy uložený snímek,
jako režimy zobrazení v Sirilu: **Lineární (jak je uloženo)**, **Logaritmus**,
**Odmocnina**, **Druhá mocnina**, **Asinh**, **AutoStretch**, **Histogram
equalisation**. Hodí se k nahlédnutí do stínů nebo kontrole pozadí, aniž byste
cokoli měnili. Dokud není zvoleno Linear, svítí „view only".

**◐ Porovnat** (podržte ho, nebo podržte B) ukáže výchozí bod: otočeno,
oříznuto a roztaženo, nic víc.

**Uložit** (vpravo): **Velikost** (plná velikost, nebo delší strana např. 2048 px
pro fóra), název a typ souboru, **Kvalita JPEG**. Řádek pod tím říká, kolik
paměti uložení potřebuje. FITS / XISF dostanou hotový (roztažený) snímek jako
32bitová desetinná čísla. **Ponechat nastavení pro další obrázek** (vlevo) dá
několika snímkům stejný vzhled, např. dílům panoramy.

| Klávesa | Akce |
|---|---|
| Ctrl+O | Otevřít snímek |
| Ctrl+S | Uložit (do zobrazeného souboru) |
| Ctrl+Shift+S | Uložit jako… |
| F | Celý náhled do okna |
| B (držet) | Porovnat s výchozím bodem |
| Ctrl+Z, Ctrl+Y | Zpět, znovu |

**Obloha a popředí** (obloha a popředí; na začátku zavřené): krajina pod
hvězdami často potřebuje obě části upravit jinak, např. méně šumu a menší
hvězdy na obloze, světlejší a teplejší popředí. Masku namalujte pomocí
**✎ Malovat** (levé tlačítko = obloha, pravé = popředí, kolečko myši = velikost
štětce) nebo ji nechte najít **◎ Rozpoznat oblohu** (obloha nahoře; opravte ji
štětcem; pro kruhové rybí oko **◯ Kruh fisheye**, kapitola 17); **Celá obloha**, **Invertovat** a **Vymazat** začnou znovu. Pak zvolte
**Obloha** nebo **Popředí** a nastavte jejich světla, teplotu, barvy a
detail. **Měkký okraj** prolne oba vzhledy na hranici; **Ukázat masku** obarví
oblohu modře a popředí oranžově (při malování vždy). Maska leží na samotném
snímku, takže při otočení nebo ořezu zůstane na místě. Uložený soubor dostane
stejné úpravy; • za názvem oddílu ukazuje, že se používají.

**↶ Zpět** a **↷ Znovu** (Ctrl+Z, Ctrl+Y nebo Ctrl+Shift+Z) vracejí a
opakují změny; tažení posuvníku nebo tah štětcem je jeden krok a popisek
tlačítka říká, co vrátí.

**✂ Ořez** ořízne přímo v obraze: ukáže se celý snímek s rámečkem; tažením
nakreslíte nový rámeček, tažením za hrany nebo rohy ho upravíte, tažením
uvnitř ho posunete. **✂ Ořez: hotovo** ukáže výsledek. Tlačítka tvarů a
posuvníky ořezu v **Otočit a oříznout** nastavují tentýž rámeček.

**Předvolby** (nahoře, pod snímkem): napište název a **Uložit** uloží současný
vzhled: roztažení, vyvážení bílé, světla, křivky, barvy, detail a vzhled oblohy
a popředí, ale ne otočení, ořez a velikost exportu (ty patří k jednomu
snímku). **Předvolba** použije uložený vzhled na otevřený snímek (Ctrl+Z ho
vrátí); **Smazat** smaže předvolbu s napsaným názvem. Předvolby jsou ve složce
nastavení (`presets`).

**Redukce šumu** pracuje s vlnkami: obrázek se rozloží na vrstvy jemných až
hrubých detailů a v každé se zmenší drobné kmitání, které tvoří šum, zatímco
hvězdy, hrany a struktura zůstanou. Šum se měří oblast po oblasti, takže to
funguje na lineárních i roztažených obrázcích. **Redukce šumu** upravuje jas,
**Barevný šum** jen barvu (jas zůstane přesně jako byl).

**Lineární uložení.** Když je obrázek lineární (je zvolené roztažení), volba
**Lineární (ke zpracování)** pod názvem souboru uloží jen lineární kroky:
otočení a ořez, extrakci pozadí, kalibraci barev a vyvážení bílé, pro další
zpracování jinde. **Roztažené (k prohlížení)** uloží vše, jak je zobrazeno.

## 17. Timelapse

*Novinka ve verzi 0.4 (ve vývoji).* Karta **Timelapse** dělá ze série snímků
video: krajinu pod noční oblohou fotografovanou ze stativu, kde se obloha
otáčí nad nehybným popředím. Pomalý zoom nebo posun přes snímek, jako by
fotoaparát jel na slideru, dodá pohyb kamery.

**Projekty:** **⬇ Uložit projekt…** uloží snímky, vzhledy, masku (i kruh
fisheye) a všechna nastavení videa do souboru `.tlproj`;
**⇪ Otevřít projekt…** (Ctrl+O) vše vrátí, i když se složka mezitím
přesunula. Ctrl+S uloží rovnou do otevřeného projektu. „Start from the
Post-processing tab's look“ se ukládá jen jako přepínač: použije obrázek,
který je právě otevřený v Post-processing.

**Snímky:** přidejte soubory nebo složku; přehrávají se v pořadí názvů
souborů (číslování fotoaparátu). Kliknutím se snímek zobrazí uprostřed;
◀ ▶ listují. U snímků RAW volba **RAW přes** určuje, jak se načtou:
**vestavěný** (výchozí) je rychlý a zachází se všemi snímky stejně;
**rawtherapee** použije úpravy uložené v souborech `.pp3` programu
RawTherapee vedle snímků, ale trvá několik sekund na snímek (asi desetkrát
déle); **auto** použije RawTherapee jen u snímků, které soubor `.pp3` mají.
Karta ukazuje, kolik snímků ho má. RawTherapee zapisuje `.pp3` ke každé
fotce otevřené v jeho prohlížeči souborů, takže soubor neznamená, že fotka
byla upravena.

**Look (vzhled):** stejné posuvníky světla a barev jako v Post-processing
(expozice, kontrast, světla, střední tóny, stíny, bílé, černé, teplota,
odstín, sytost, živost), zvlášť pro **Celý snímek**, **Obloha**
(oblohu) a **Popředí**. Obloha a popředí se přidávají k vzhledu
celého snímku, jen ve své části masky; • označuje změněné části. Vybraný
snímek ukazuje vzhled (**Ukázat vzhled**; vypnuto = snímek tak, jak je).
Odstranění blikání a pohyb kamery jsou vidět jen ve videu.

**Maska:** jako v Post-processing (kapitola 16): **✎ Malovat**,
**◎ Rozpoznat oblohu**, **Celá obloha**, **Invertovat**, **Vymazat**, **Měkký okraj**,
**Ukázat masku**. Jedna maska platí pro všechny snímky, protože na stativu se
popředí nehýbe.

Pro celooblohový timelapse s kruhovým rybím okem namířeným nahoru najde
**◯ Kruh fisheye** osvětlený obrazový kruh a oblohou udělá kotouč uprostřed;
prstenec kolem (obzor, stromy, světla) je popředí. **Kruh oblohy** nastaví,
kolik poloměru kruhu je obloha, a velká hodnota **Měkký okraj** (až 25 %) udělá
z hranice radiální přechod. Zbytek opravte štětcem, např. dalekohled nebo
člověka, který zasahuje do oblohy.

**Video:**

| Nastavení | Co dělá |
|---|---|
| **Odstranění blikání** | Vyrovná skoky jasu i barvy mezi snímky (kroky expozice, měnící se světlo, vyvážení bílé fotoaparátu). **Mírné** porovná každý snímek se sousedy (**Vyhlazení**, ve snímcích); **Silné** jich použije třikrát víc. Pomalé změny, jako soumrak, zůstanou. **I barvy** vyrovná každou barvu zvlášť; **Po oblastech** to dělá na hrubé mřížce přes snímek, takže změny ve velkém měřítku v jedné části (světelné znečištění, projíždějící světlo, svítání na jedné straně) se vyrovnají tam, kde vznikly. Obojí funguje pro běžné objektivy i rybí oko. Měření přečte každý snímek ještě jednou. |
| **Začít od vzhledu z karty Dodatečné úpravy** | Každý snímek nejdřív dostane nastavení snímku otevřeného v Post-processing (roztažení, vyvážení bílé, světla, křivky, barvy, detail; ne jeho otočení a ořez), potom vzhled nastavený zde. Hodnoty měřené automatickým roztažením se vezmou z prvního snímku a platí pro všechny, takže video nebliká. |
| **Posun jako na slideru** | Jako by fotoaparát jel na motorizovaném slideru: video ukazuje část snímku o tolik procent menší (až 20 %) a posouvá ji přes snímek, **Zleva doprava**, **Zprava doleva**, **Nahoru** nebo **Dolů**. Posun je plynulý, ne po celých pixelech. |
| **Pomalý zoom** | Pomalé přiblížení během celého videa; lze kombinovat s posunem. |
| **Plynulý rozjezd a dojezd** | Posun i zoom se na začátku rozjedou a na konci zpomalí; vypnuto: stále stejnou rychlostí. Náhled ukáže obrys začátku (jasný) a konce (slabý) videa. |

**Výstup:**

| Nastavení | Co dělá |
|---|---|
| **Formát** | **MP4** (H.264) a **WebM** (VP9) vyrábí ffmpeg. Nainstalujte ho (ffmpeg.org; Windows: `winget install ffmpeg`; Linux: správce balíčků; macOS: Homebrew) nebo ho dejte vedle programu; karta ukáže, kde ho našla. **Animovaný GIF** a **Číslované obrázky** (číslované snímky TIFF nebo PNG 16 bitů, JPEG) nic dalšího nepotřebují. GIFy hvězdných polí jsou velké (asi 1 MB na snímek při 720p; MP4 je 5–10× menší), hodí se tedy na krátké ukázky. |
| **Snímková frekvence** | 12–60 snímků za sekundu. |
| **Rozlišení** | Jako snímky, 4K, 1440p, 1080p nebo 720p; nikdy se nezvětšuje. Do 1080p se snímky RAW čtou v poloviční velikosti, což je rychlejší a stále dost ostré. |
| **Tvar** | Tvar videa; snímek se na něj ořízne a náhled ukáže obrys ořezu. **Jako snímky** (bez ořezu), **16:9**, **3:2** a **4:3** (řídí se orientací snímků), **1:1 čtverec** (celý kruh celooblohového fisheye) nebo **9:16 na výšku** (telefony). V 1080p: 1920 × 1080, 1620 × 1080, 1440 × 1080, 1080 × 1080 a 1080 × 1920. S kruhem fisheye karta upozorní, když tvar odřízne část oblohy. |
| **Kvalita** | Pro MP4, WebM a snímky JPEG: vyšší = lepší obraz, větší soubor. |
| **Přehrát dopředu, pak zpět** | Na konci snímky ještě jednou pozpátku, takže video plynule navazuje ve smyčce. |
| **Podržet poslední snímek** | Nechá poslední snímek pár sekund stát. |

Pod nastavením: kolik je snímků, jak dlouho video poběží a jeho velikost v
pixelech. **▶ Vytvořit video** pracuje na pozadí, nejvýš po osmi snímcích
(po méně, když chybí paměť), s
ukazatelem průběhu (uplynulý a zbývající čas) a **✖ Zrušit** (nic se
neponechá). Potom zpráva ukáže snímky, velikost, snímkovou frekvenci a délku
videa, jak dlouho to trvalo a soubor s jeho velikostí, případně co se
nepovedlo. Například z 69 snímků RAW (24 MP) vzniklo video MP4 1080p s
odstraněním blikání asi za půl minuty na stolním PC.
