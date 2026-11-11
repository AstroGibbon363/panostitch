# Panorama Stitcher: uživatelská příručka

*Další jazyky: [English](manual_en.md) · [Deutsch](manual_de.md)*

Program pro počítač, který spojí překrývající se snímky do jedné panoramy nebo
mozaiky: JPEG, TIFF, PNG, RAW z fotoaparátu a astronomické snímky FITS. Každý
krok proběhne až po stisknutí příslušného tlačítka a každý výsledek si můžete
před pokračováním prohlédnout a opravit. Druhá karta skládá snímky hvězd do
oblohy s nízkým šumem nebo do hvězdných stop ([Skládání hvězd](#13-skládání-hvězd)).

Ovládací prvky programu jsou pojmenované anglicky. V této příručce jsou proto
uvedeny **tučně v angličtině**, přesně tak, jak je uvidíte na obrazovce.

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
```

Poznámky:

- Program automaticky využívá všechna jádra procesoru.
- Okno se otevře do jedné až dvou sekund; nic se předem nepřipravuje.
- Pokud se okno neobjeví, spusťte program z terminálu: vypíše tam chybová
  hlášení.
- Přepínače vzhledu (Dark, Modern, Simple) se ukládají do `ui.toml` ve složce
  nastavení: `~/.config/panostitch` na Linuxu,
  `~/Library/Application Support/panostitch` na macOS,
  `%APPDATA%\panostitch` na Windows.

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

Přepínače vpravo nahoře mění vzhled: **Dark** (tmavý), **Modern** (zaoblenější,
vzdušnější ovládání) a **Simple** (jednoduchý). Jednoduchý režim skryje podrobné
ovládání (nastavení hledání a párování bodů, prolínání, roztažení, posouvání)
a ponechá, co potřebuje většina panoram: snímky, předvolbu, **Run all steps**,
projekci s ohniskovou vzdáleností, natočením (yaw), sklonem (pitch) a
klopením (roll), expozici, **Even out**, ořez, náhled a uložení. Skrytá
nastavení si ponechají hodnoty z předvolby. Program si přepínače pamatuje.

---

## 4. Rychlý začátek

**Fotografická panorama:**

1. Nechte zapnuté **half size**. Je to rychlejší a na kontrolu výsledku to
   bohatě stačí.
2. Zvolte **Preset** podle námětu (viz [Předvolby](#50-předvolby)).
3. Napište složku se snímky do pole pro cestu a stiskněte **Enter**, nebo
   klikněte na **Add…** a snímky vyberte. **Na pořadí nezáleží.**
4. Klikněte na **Run all steps** a počkejte, až stavový řádek ukáže *Done*.
5. Vpravo zvolte **projekci**: **cylindrical** pro jednu řadu, **spherical**
   pro více řad nebo pro záběry sahající hodně nahoru či dolů. Je-li výsledek
   tmavý, klikněte na **Auto exposure**; je-li horizont prohnutý, upravte
   **Pitch**. Tlačítkem **Auto crop** odříznete zubaté okraje.
6. Klikněte na **Save full resolution…**. Chcete-li se k práci později vrátit,
   klikněte i na **Save project…**.

**Mozaika hlubokého vesmíru ze souborů FITS:**

1. Zvolte předvolbu **deep-sky mosaic**.
2. Načtěte dlaždice FITS (složené nebo jednotlivé snímky). Pro zobrazení se
   automaticky roztáhnou.
3. Klikněte na **Run all steps** a potom na **Auto stretch**, aby roztažení
   odpovídalo celé mozaice. Podle potřeby ho dolaďte.
4. Uložte jako **`.fits`**, pokud chcete lineární data dál zpracovávat, nebo
   jako **`.tif`**, pokud chcete roztažený obrázek tak, jak ho vidíte.

---

## 5. Krok za krokem

**Zadávání čísel:** vedle každého posuvníku je číselné pole. Klikněte do něj,
napište hodnotu a stiskněte **Enter**. Hodnota se zaokrouhlí na krok a rozsah
posuvníku.

### 5.0 Předvolby

Nabídka **Preset** (levý sloupec, pod *RAW via*) nastaví najednou všechny
hodnoty párování a skládání pro daný typ námětu. Každou hodnotu lze potom
dál měnit.

| Předvolba | Pro | Hlavní nastavení |
|---|---|---|
| **day landscape** | denní panoramy, fotoaparát otáčený na místě | slabé odšumění, homografie, cylindrická projekce |
| **astro / night landscape** | noční obloha, Mléčná dráha nad krajinou | silné odšumění, volnější párování, sférická projekce, +2 EV (pak **Auto exposure**) |
| **deep-sky mosaic** | mozaiky hvězdných polí z dalekohledu, např. FITS | málo odšumění (hvězdy jsou malé), model podobnosti (posun + rotace pole), bez zpřesnění, přímočará projekce; použijte **Auto stretch** |
| **macro / microscope** | preparát nebo stolek posouvaný pod pevným fotoaparátem | model posunu (snímky se jen posouvají), bez zpřesnění, přímočará projekce |
| **scan / flat document** | mapy, obrazy, dokumenty fotografované po částech | afinní model, bez zpřesnění, přímočará projekce |

### 5.1 Snímky (Images)

| Prvek | Co dělá |
|---|---|
| **Add…** | Otevře dialog pro výběr souborů. Vybrat můžete více souborů (Ctrl/Shift + klik). |
| **Pole pro cestu** | Napište cestu k souboru nebo ke **složce** a stiskněte Enter. Složka načte všechny snímky v ní, seřazené podle jména. Soubory pojmenované `panorama…` (dřívější výsledky) se přeskočí. |
| **Seznam snímků** | Kliknutím snímek vyberete a zobrazíte. **Ctrl + klik** přidá snímek do výběru nebo ho z něj odebere, **Shift + klik** vybere rozsah a **Ctrl+A** (s myší nad seznamem) vybere vše. Seznam se posouvá kolečkem myši. |
| **Remove** | Odstraní vybrané snímky. Totéž udělá klávesa **Delete**, když je myš nad seznamem. |
| **Clear all** | Odstraní všechny snímky. |
| **▲ / ▼** | Posune vybrané snímky v seznamu nahoru nebo dolů. Na pořadí záleží jen v režimech párování *sequential* a *grid*. |
| **Reference** | Snímek, kolem kterého se ostatní umisťují; zůstane nezkreslený. **auto (centre)** zvolí snímek uprostřed skupiny, což obvykle dává nejvyrovnanější výsledek. |
| **RAW via** | Který převodník RAW použít, viz [Soubory RAW](#8-soubory-raw). Platí pro snímky přidané poté. |
| **half size** | Načte soubory RAW a soubory FITS z barevných kamer v poloviční velikosti: asi 4× rychleji a se čtvrtinou paměti. Platí pro snímky přidané poté. Chcete-li výsledek v plném rozlišení, vypněte ho před načtením. |

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
| **Detection size (px)** | 2000 | Před hledáním se snímek zmenší na tuto velikost (delší strana). Větší je přesnější, ale pomalejší. |
| **Max keypoints** | 5000 | Horní mez na snímek. Body se rozmístí rovnoměrně po celém snímku. |
| **Corner threshold** | 0,001 | Minimální síla rohu. Nižší najde i slabší rohy (užitečné u oblohy nebo mlhy), vyšší jen výrazné. |
| **Denoise (σ px)** | 1,5 | Před hledáním rozmaže šum snímače. Pro **noční snímky** a zašuměné snímky s vysokým ISO ho zvyšte (2–3); pro hvězdná pole a čisté denní snímky s jemnými detaily ho snižte (0,5–1). |

Klikněte na **Detect features**. Náhled ukáže aktuální snímek s body jako
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
| **all pairs** | Výchozí. Každý snímek se zkouší s každým, takže **na pořadí nezáleží** a i více řad funguje samo. U mnoha snímků trvá déle (74 fotek ≈ 35 s). |
| **sequential** | Jedna řada, snímky v pořadí pořízení. Každý snímek se páruje jen s následujícím. Nejrychlejší. |
| **grid** | Více řad ve známém pořadí: **Grid columns** snímků v řadě, řada za řadou. Zkoušejí se sousedé vpravo, dole a úhlopříčně. Pokud byla každá druhá řada fotografována opačným směrem, zapněte **snake**. |

**Nastavení:**

| Prvek | Výchozí | Význam |
|---|---|---|
| **Grid columns** | 5 | Snímků v řadě (jen v režimu grid). |
| **Ratio test** | 0,8 | O kolik lepší musí být nejlepší shoda než druhá nejlepší. Nižší je přísnější (méně, ale spolehlivějších shod), vyšší dá více shod. |
| **Max distance** | 0,35 | Jak odlišně mohou dva body vypadat, aby se ještě spárovaly. |
| **RANSAC tol. (px)** | 5,0 | Jak daleko (v pixelech) smí shoda ležet od modelu, aby se ještě počítala jako inlier. |
| **Min. inliers** | 20 | Nejmenší počet inlierů, aby se dvojice počítala jako skutečný překryv. |
| **Model** | homography | Jak se jeden snímek zobrazí na druhý, viz níže. |
| **refine** | zapnuto | Globální zpřesnění, viz níže. |

**Kdy se dvojice počítá jako skutečný překryv:** potřebuje alespoň *Min.
inliers* inlierů, jasná většina jejích shod musí souhlasit a transformace musí
být věrohodná (nesmí snímek převrátit ani extrémně roztáhnout). Tím se
odfiltrují falešné shody mezi snímky, které se ve skutečnosti nepřekrývají.

**Modely:**

| Model | Kdy použít |
|---|---|
| **homography** | Běžné panoramy: fotoaparát otáčený na místě. Výchozí. |
| **affine** | Skeny nebo fotografie plochého předmětu z velké dálky. |
| **similarity** | Jen posun, otočení a rovnoměrné zvětšení, např. mozaiky z dalekohledu s rotací pole. Stabilní i při malém počtu shod. |
| **translation** | Jen posun, např. skener nebo stolek mikroskopu. Nejrobustnější, nejméně pružný. |

**Umístění a globální zpřesnění (refine):** snímky se propojí přes své
nejsilnější dvojice, počínaje referenčním snímkem. Se zapnutým **refine** (jen
u modelu homography) se pak všechny dvojice řeší najednou jako jeden otáčející
se fotoaparát. Malé chyby se tak rozloží rovnoměrně, místo aby se sčítaly, a
uzavřou se smyčky mezi řadami; to odstraní většinu „duchů“. Zároveň se odhadne
soudkovité zkreslení objektivu (výrazné u širokoúhlých objektivů) a při
vykreslení se opraví. Dvojice, které do společného řešení nezapadají, se
**automaticky vyřadí**, protože jde obvykle o falešné shody nebo o něco, co se
pohnulo. Stavový řádek ukáže, kolik jich bylo vyřazeno, a **typickou chybu
dvojic** (1–3 px je dobré).

**Tlačítka:**

- **Match**: spáruje dvojice zvolené v nabídce Pairs (paralelně), potom
  snímky umístí a zpřesní.
- **Re-match pair**: znovu spáruje jen zobrazenou dvojici s aktuálním
  nastavením.
- **Discard pair**: přestane používat zobrazenou dvojici (pro shodu, která je
  zjevně špatná); umístění se přepočítá bez ní.
- **Run all steps**: body, párování, umístění a náhled najednou.

Pohled **Pair matches** ukazuje dva snímky vedle sebe:

- **zelené čáry**: inliery (správné shody)
- **červené čáry**: outliery. Skryjete je přepínačem **outliers**.

Nabídka **Pair** vypisuje přijaté dvojice s počtem inlierů. Dvojice označené
*rejected* neprošly některou kontrolou; text pod nabídkou uvádí proč.

**Jak vypadá dobrý výsledek:** desítky zelených čar nebo více, všechny
popisující stejný posun, a průměrná chyba 1–2 px.

**Nepropojené snímky:** snímek bez přijaté dvojice k ostatním je v seznamu
označen `UNLINKED` a **do panoramy se nezahrne**, aby ji nepokazil. Zapněte
**show unlinked images** (pravý sloupec), aby se takové snímky ukázaly v řadě
pod panoramou, a umístěte je ručně pomocí **Ctrl + tažení**.

### 5.4 Krok 3: Projekce (Projection)

Jak jednotlivé projekce vypadají, popisuje kapitola [Projekce](#7-projekce).

| Prvek | Význam |
|---|---|
| **Nabídka projekce** | 14 projekcí, viz [Projekce](#7-projekce) |
| **Focal (× auto)** | Ohnisková vzdálenost se odhadne ze shod. Tento posuvník ji násobí. Vypadá-li panorama do stran stlačená nebo roztažená, zkuste hodnoty 0,8 až 1,2. |
| **Yaw (°)** | Otočí celou panoramu doleva nebo doprava: určuje, který směr bude uprostřed. Hodí se u 360° panoram (např. aby jádro Mléčné dráhy bylo uprostřed, ne na okraji). |
| **Pitch (°)** | Nakloní celou panoramu nahoru nebo dolů. Slouží k **narovnání prohnutého horizontu** (například když fotoaparát mířil dolů). |
| **Roll (°)** | Otočí celou panoramu. Slouží k vyrovnání šikmého horizontu. |
| **Reset focal / yaw / pitch / roll** | Vrátí tyto čtyři posuvníky na výchozí hodnoty. |

Titulek náhledu ukazuje velikost panoramy, ohniskovou vzdálenost v pixelech,
zorný úhel referenčního snímku a typickou chybu zarovnání.

Tyto prvky mění náhled okamžitě (jakmile náhled existuje).

### 5.5 Krok 4: Skládání (Compose)

| Prvek | Význam |
|---|---|
| **Blend** | **smart** (výchozí): vícepásmové prolínání s odstraněním duchů. Jemné detaily pocházejí v každém místě z jednoho snímku, takže nevzniknou dvojité obrazy lidí, světel ani blízkých předmětů. Rozdíly jasu se prolnou v širokém pásu, takže nevzniknou ostré spoje. Kde se překrývající snímky neshodují (něco se pohnulo), převezme se celá oblast z jednoho snímku. **feather**: prosté plynulé míchání; kde se něco pohnulo, ukáže duchy. **seam**: každý pixel z nejvíce středového snímku; bez duchů, ale s ostrými spoji. **average**: prostý průměr; hlavně pro kontrolu zarovnání. |
| **Preview size (px)** | Velikost náhledu (delší strana). Větší je ostřejší, ale pomalejší. Při přiblížení se detail stejně dopočítá (viz [Pohyb v náhledu](#6-pohyb-v-náhledu)). |
| **Exposure (EV)** | Zesvětlí nebo ztmaví celou panoramu, od −3 do +8 EV (+1 EV = dvakrát jasnější). Tmavé astro panoramy často potřebují +1,5 až +4. |
| **Brightness (sel.)** | Jasový faktor **vybraných** snímků. |
| **show unlinked images** | Ukáže i snímky, které se nepodařilo propojit, v řadě pod panoramou, aby je šlo umístit ručně. |
| **Auto exposure** | Nastaví **Exposure** podle aktuálního náhledu: typický jas na střední šedou, aniž by se přepálilo víc než nejjasnějších 0,5 % (hvězdy, lampy). |
| **Even out** | Spočítá jasové faktory snímků tak, aby si překryvy odpovídaly. |
| **Reset moves** | Zruší všechna ruční posunutí. |
| **Auto crop** | Ořízne panoramu na největší obdélník, který je celý pokrytý snímky, takže zmizí černé zubaté okraje. Náhled pak ukazuje oříznutý výsledek a **Save full resolution…** ho uloží. Stavový řádek uvede, kolik panoramy zůstalo. Nejdřív narovnejte horizont (Pitch, Roll); z prohnuté panoramy zbude jen úzký pruh. |
| **Uncrop** | Zruší ořez. Ořez se zruší i automaticky, když změníte cokoli, co snímky posune (projekci, ohnisko, yaw, pitch, roll, ruční posuny, párování). |
| **Nudge ← ↑ ↓ →** | Posune aktuální snímek o jeden pixel náhledu (viz níže). |

V pohledu **Panorama** je obrys každého snímku vykreslen a očíslován:

- **oranžově**: aktuální snímek
- **světle modře**: další vybrané snímky
- **azurově**: referenční snímek
- **bíle**: ostatní snímky
- **přerušované žluté čáry**: vazby použité k umístění snímků

Přepínačem **overlays** v liště je vypnete a uvidíte samotnou panoramu.

**Kliknutím do obrysu** snímek vyberete (funguje i s vypnutými překryvy).

**Ruční posouvání snímků:** držte **Ctrl** a táhněte snímek, nebo použijte
šipky či tlačítka **Nudge**. Snímek se posune spolu se snímky, které jsou k
němu připojené dál od reference (podél žlutých vazeb). Referenční snímek se
nehýbe; celou panoramu posunete pomocí Pitch a Roll.

### 5.6 Krok 5: Roztažení histogramu (Stretch)

Astronomické snímky (a tmavé převody RAW) jsou obvykle **lineární**: zajímavé
detaily leží v nejtmavších několika procentech rozsahu jasu a snímek vypadá
skoro černě. Roztažení histogramu je zviditelní.

| Prvek | Význam |
|---|---|
| **Histogram** | Rozložení jasu v náhledu panoramy (v pohledu Panorama) nebo v aktuálním snímku (v ostatních pohledech), v logaritmickém měřítku. **Modrá čára** značí černý bod, **bílá čára** bílý bod a **oranžová křivka** ukazuje, jak se vstupní jas (zleva doprava) převádí na výstupní (zdola nahoru). |
| **Black point** | Vše s tímto nebo nižším jasem bude černé. Nastavte ho těsně pod pozadí oblohy (levý okraj hlavního vrcholu histogramu). |
| **Midtones** | Zesvětlí (menší hodnoty) nebo ztmaví (větší hodnoty) střední tóny bez ořezání. 0,5 je neutrální. Snímky hlubokého vesmíru obvykle potřebují 0,005–0,05. |
| **White point** | Vše s tímto nebo vyšším jasem bude bílé. |
| **Auto stretch** | Nastaví všechny tři hodnoty podle aktuálního histogramu: černý bod těsně pod pozadím oblohy, střední tóny tak, aby pozadí vyšlo asi na 25 % šedi (stejný princip jako automatické roztažení v PixInsightu). |
| **Reset stretch** | Zpět bez roztažení (0 / 0,5 / 1). |

- Roztažení mění jen to, jak se snímky **zobrazují** a jak se panorama ukládá
  do TIFF, PNG nebo JPEG. Párování pracuje vždy s původními daty.
- **Soubory FITS** se při načtení automaticky roztáhnou pro zobrazení, a to
  podle všech načtených snímků FITS dohromady.
- Změna roztažení je okamžitá, nic se nepřepočítává.
- **Exposure** se uplatní před roztažením.

### 5.7 Zpět, znovu a projekty

**Zpět / znovu:** tlačítka **Undo** a **Redo** v liště, nebo **Ctrl+Z** a
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
  zobrazení (zoom, zvolený pohled, překryvy), *RAW via* a *half size* a
  ukládání souborů.

**Projekty:** **Save project…** (nahoře v levém sloupci) uloží soubor projektu
s příponou **`.panoproj`**. **Open project…** ho znovu načte. Cestu k souboru
`.panoproj` můžete také napsat do pole pro cestu a stisknout Enter, nebo s ním
program spustit: `panostitch moje_pano.panoproj`.

Projekt obsahuje:

- snímky (úplnou cestu a cestu relativní k souboru projektu)
- všechna nastavení: posuvníky, nabídky, přepínače, včetně roztažení
- výsledky párování, takže se nic nemusí párovat znovu
- ruční posuny, jasové faktory, referenční snímek a ořez
- *RAW via* a *half size*, aby se snímky načetly stejně

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
| Přiblížení | kolečko nahoru | `+` | **Zoom +** |
| Oddálení | kolečko dolů | `-` | **Zoom -** |
| Zpět na celek | | `0` nebo `Home` | **Fit** |
| Výběr snímku (pohled Panorama) | klik do jeho obrysu | | |
| Zobrazit / skrýt obrysy, čísla, vazby | | | přepínač **overlays** |
| Posun snímku (pohled Panorama) | **Ctrl + tažení** | šipky (`Shift` = 10 kroků) | **← ↑ ↓ →** |
| Zpět | | `Ctrl+Z` | **Undo** |
| Znovu | | `Ctrl+Y` nebo `Ctrl+Shift+Z` | **Redo** |

Tlačítka **Image + keypoints**, **Pair matches** a **Panorama** přepínají mezi
třemi pohledy.

**Ostré přiblížení:** náhled je zmenšený obrázek. Když panoramu přiblížíte a
chvíli počkáte, viditelná oblast se znovu vykreslí v rozlišení obrazovky z
původních snímků. Můžete tak zkontrolovat jemné detaily a spoje.

---

## 7. Projekce

Panorama zabírá víc, než dokáže ukázat plochá fotografie, takže ji program musí
*promítnout* do roviny. Každá projekce něco zkresluje:

| Projekce | Vypadá jako | Nejlepší pro | Omezení |
|---|---|---|---|
| **rectilinear** (přímočará) | běžná fotografie: rovné čáry zůstanou rovné | úzké panoramy (do ~100°), architektura, mozaiky z dalekohledu | se šířkou záběru se okraje extrémně roztahují |
| **cylindrical** (válcová) | vodorovné čáry se ohýbají, svislé zůstanou svislé | většina vodorovných panoram | ne pro vysoké záběry (horní a dolní okraj se roztahují) |
| **spherical** (sférická, ekvidistantní válcová) | jako válcová, ale méně roztažená svisle | velmi široké a 360° panoramy, prohlížeče panoram | rovné čáry se ohýbají |
| **mercator** (Mercatorova) | mezi válcovou a sférickou | široké panoramy s určitou výškou | roztahuje se u horního a dolního okraje |
| **stereographic** (stereografická) | podobná rybímu oku, tvary zůstávají kulaté | velmi široké zorné úhly, kreativní záběry | silné zakřivení |
| **transverse_cylindrical** (příčná válcová) | válcová položená na bok: svislé čáry se ohýbají, vodorovné zůstávají rovné | **svislé panoramy** (od horizontu k zenitu), vztyčený oblouk Mléčné dráhy | ne pro široké vodorovné záběry (přetečou přes horní a dolní okraj) |
| **transverse_mercator** (příčná Mercatorova) | jako příčná válcová, do stran méně roztažená | svislé panoramy s určitou šířkou | jako výše |
| **fisheye** (rybí oko, ekvidistantní) | kulatý obraz, vzdálenost od středu úměrná úhlu | **celooblohové pohledy**: Pitch nakloňte tak, aby byl zenit uprostřed | okraje stlačené |
| **fisheye_equisolid** (rybí oko, plochojevné) | kulatý obraz zachovávající poměr ploch | celooblohové pohledy, jak vypadá většina objektivů typu rybí oko | okraje stlačené ještě víc |
| **little_planet** (malá planeta) | země uprostřed, obloha kolem ní jako prstenec | 360° panoramy se zemí, kreativní | panorama musí sahat hodně dolů |
| **tunnel** (tunel) | obloha uprostřed, horizont kolem ní | 360° noční obloha, stopy hvězd, stromy zespodu | panorama musí sahat hodně nahoru |
| **orthographic** (ortografická) | zeměkoule viděná zvenku | kreativní; ukazuje polovinu koule | okraje jsou velmi stlačené |
| **hammer** (Hammerova–Aitoffova) | oválná mapa celé koule se správným poměrem ploch | panoramy celé koule jako jeden obraz, celooblohové hvězdné mapy | tvary u okrajů zkreslené |
| **mollweide** (Mollweideova) | oválná plochojevná mapa, u pólů plošší | jako Hammerova; klasická celooblohová mapa v astronomii | tvary u okrajů zkreslené |

Vypadá-li horizont jako úsměv nebo zamračení, upravte **Pitch**. Je-li šikmý,
upravte **Roll**. **Yaw** otáčí pohled do stran. U rybího oka, malé planety a
tunelu určuje Pitch, co bude uprostřed.

---

## 8. Soubory RAW

Podporováno: **Canon CR2 a CR3, Nikon NEF a NRW, Sony ARW a SRF, DNG, Fuji RAF,
Olympus ORF, Panasonic RW2, Pentax PEF, Samsung SRW** a další. Přesná podpora
fotoaparátů závisí na dekodéru; vestavěný zná většinu fotoaparátů posledních
dvou desetiletí, LibRaw (je-li nainstalované) asi 1250 fotoaparátů.

**RAW via** volí dekodér:

| Volba | Dekodér | Charakter |
|---|---|---|
| **auto** | první, který uspěje, v pořadí níže; má-li snímek soubor `.pp3`, nejdřív RawTherapee | doporučeno |
| **built-in** | vlastní dekodér programu | vždy k dispozici, rychlý; neutrální převod s barvami a vyvážením bílé fotoaparátu, bez automatického zesvětlení, takže všechny snímky mají stejnou expozici |
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
úpravy (stejný profil `.pp3`), nebo použijte **built-in** a barvy opravte až
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
| **Surová data barevné kamery (Bayer)** | snímky ze ZWO, ToupTek, QHY… s hlavičkou `BAYERPAT` (RGGB, BGGR, GRBG, GBRG) | rozloží se na barvy (debayer); s **half size** jeden barevný pixel na buňku 2×2 (rychlé, čisté), jinak interpolace v plné velikosti |
| **Pořadí řádků** | `ROWORDER` = `TOP-DOWN` nebo `BOTTOM-UP` (výchozí ve FITS) | snímek se zobrazí správně otočený |

Nepodporováno: **dlaždicově komprimované soubory (`.fz`)**. Nejdřív je
rozbalte, např. programem `funpack`. Další části souboru (tabulky, profily ICC)
se ignorují; použije se první obrázek.

**Poznámky k mozaikám hlubokého vesmíru:**

- Použijte předvolbu **deep-sky mosaic**. Dlaždice z dalekohledu se jen
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

Klikněte na **Save full resolution…** a zvolte název souboru. Typ souboru se
řídí příponou:

| Přípona | Výsledek |
|---|---|
| `.tif` / `.tiff` | 16 bitů na kanál, nejlepší pro další úpravy; uloží se **tak, jak to vidíte** (s expozicí a roztažením) |
| `.png` | 16 bitů na kanál; tak, jak to vidíte |
| `.jpg` | 8 bitů na kanál, menší soubor; tak, jak to vidíte |
| `.fits` / `.fit` / `.fts` | 32bitová desetinná čísla, tři barevné roviny; **lineárně** (expozice a jasové faktory se uplatní, ale **bez roztažení** a bez ořezu), pro astronomický software |

Jsou-li všechny načtené snímky FITS, navržený název souboru končí na `.fits`.

Panorama se vykreslí z načtených snímků v jejich plné velikosti: v polovičním
rozlišení fotoaparátu, pokud bylo při načítání zapnuté **half size**.

Panoramy širší nebo vyšší než 40 000 px se odmítnou. Téměř vždy to znamená, že
je některá dvojice spárovaná špatně nebo že projekce není vhodná; zkuste
cylindrical nebo spherical.

---

## 11. Řešení problémů

| Problém | Co zkusit |
|---|---|
| **Okno se neobjeví** | Spusťte program z terminálu a přečtěte si tam chybové hlášení. Na Linuxu potřebuje OpenGL: ověřte, že je nainstalovaný grafický ovladač. |
| **„Busy – wait…“** | Krok ještě běží; počkejte, až se stavový řádek změní. |
| **Snímek se při načítání přeskočí** | Stavový řádek uvede soubor a důvod. U RAW zkuste jiný převodník v **RAW via**. Soubory FITS `.fz` nejdřív rozbalte. |
| **Panorama je skoro černá** | Fotografie: klikněte na **Auto exposure** nebo zvyšte **Exposure (EV)** (noční záběry často potřebují +1,5 až +4), nebo snímky upravte v RawTherapee (jejich soubory `.pp3` se pak použijí). FITS a jiná lineární data: klikněte na **Auto stretch**. |
| **Snímky FITS jsou po načtení černé nebo bílé** | Automatické roztažení při načtení vychází ze všech snímků FITS; jeden silně pře- nebo podexponovaný snímek ho může rozhodit. Takové snímky odstraňte a klikněte na **Auto stretch** (nebo na **Reset stretch** a nastavte ho ručně). |
| **Snímky FITS jsou zelené** | Surová data barevných kamer nemají vyvážení bílé. Nejdřív je složte a barevně zkalibrujte, nebo barvy výsledku upravte v astronomickém softwaru. |
| **Málo žlutých bodů** | Snižte **Corner threshold**; zvyšte **Max keypoints**; zvyšte **Detection size**. |
| **Body hlavně na trávě, obloze nebo šumu** | Zvyšte **Denoise** na 2–3. |
| **Snímky označené UNLINKED** | Prohlédněte si je: opravdu se překrývají s jiným snímkem? Zvyšte **Ratio test** (0,85–0,9) a **Max distance** (0,4), snižte **Min. inliers**, zvyšte **Denoise** a znovu klikněte na **Match**. Pokud to nepomůže, zapněte **show unlinked images** a umístěte je pomocí **Ctrl + tažení**. |
| **Mraky, voda nebo jiné pohyblivé věci** | Mezi snímky se mění, takže je nelze spolehlivě spárovat (snímky jen s mraky obvykle zůstanou nepropojené). Takové části foťte rychle po sobě, nebo je umístěte ručně. |
| **Použila se špatná dvojice** (čáry všemi směry, snímky na špatném místě) | Vyberte ji v nabídce **Pair** a klikněte na **Discard pair**. Se zapnutým **refine** se většina takových dvojic vyřadí automaticky. |
| **Duchové (zdvojené předměty)** | Nechte zapnuté **refine** a použijte **Blend → smart** (výchozí). Zbývající problémy pocházejí z věcí, které se mezi snímky změnily (např. svítilna rozsvícená jen na jednom snímku), nebo z **paralaxy**: blízké předměty se vůči pozadí posunou, když se fotoaparát neotáčí kolem objektivu (tzv. nodálního bodu). |
| **Viditelné jasové skoky** | **Even out**, potom jednotlivé snímky dolaďte pomocí **Brightness (sel.)**. |
| **Prohnutý horizont** | Upravte **Pitch** (a **Roll**, je-li šikmý). |
| **Silně roztažený horní nebo dolní okraj** | Použijte projekci **spherical**; stavový řádek ji navrhne, když panorama sahá hodně nahoru nebo dolů. |
| **Panorama stlačená nebo roztažená** | Upravte **Focal (× auto)**. |
| **Obrovská, zkreslená panorama** | Špatná dvojice (zkontrolujte dvojice), nebo použijte **spherical** / **cylindrical** místo rectilinear. |
| **Pomalé nebo dochází paměť** | Před načtením zapněte **half size**; snižte **Preview size**. Snímky v plném rozlišení potřebují zhruba 12 bajtů na pixel, asi 300 MB na 24MP snímek. U mnoha snímků trvá párování *all pairs* déle; *grid* je rychlejší, když rozložení znáte. |
| **Auto crop ponechá jen úzký pruh** | Panorama je prohnutá. Nejdřív ji narovnejte pomocí **Pitch** a **Roll** (a zvolte vhodnou projekci), potom znovu **Auto crop**. |
| **Projekt se otevře bez některých snímků** | Stavový řádek vypíše chybějící soubory. Byly přesunuty nebo přejmenovány, a ne spolu se souborem projektu. Vraťte je zpět, nebo je přidejte znovu a spárujte. |
| **Zpět nic neudělá** | Zpět pracuje s ustálenými změnami; po poslední změně chvíli počkejte. Zatímco běží krok (*Busy*), čeká. |
| **Všechno je pomalé** | Před načtením zapněte **half size** (RAW a FITS z barevných kamer); snižte **Preview size**. Na kartě skládání **Half size** v RAW conversion. |

---

## 12. Tipy pro dobré panoramy

- **Překryv** sousedních snímků 30–50 %, i mezi řadami. U mozaik z dalekohledu
  obvykle stačí 15–25 %.
- **Otáčejte fotoaparátem** kolem objektivu, ne kolem sebe. Pokud to jde,
  použijte stativ, ideálně s panoramatickou hlavou.
- **Zamkněte expozici, vyvážení bílé a ostření** (ruční režim), aby si všechny
  snímky odpovídaly.
- Vyhněte se pohyblivým předmětům v překryvech, nebo je zachyťte jen na jednom
  snímku.
- **Držte fotoaparát vodorovně.** Nakloněný fotoaparát ohne horizont (oprava
  pomocí Pitch a Roll).
- U dlouhých sérií nejdřív zkontrolujte výsledek se zapnutým **half size**.
- Pokud načítáte snímky po složkách, neukládejte výsledek do složky se
  zdrojovými snímky. Soubory pojmenované `panorama…` se přeskakují, jiné názvy
  ale ne.

---

## 13. Skládání hvězd

Druhá karta, **Star stacking (landscape / trails)**, spojí mnoho snímků téže
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

### 13.1 Rychlý start

1. Zvýrazněte **Star images**, klikněte na **Add…** a vyberte snímky oblohy
   (nebo zadejte cestu k souboru či složce do pole pro cestu a stiskněte Enter).
2. Zvýrazněte **Sky region**, zvolte **Boundary line** a v náhledu základního
   snímku táhněte čáru podél horizontu, nebo klikněte na **Detect sky** a
   masku opravte štětcem. Hvězdy se hledají jen na obloze.
3. Klikněte na **Quick preview**: malý, rychlý výsledek z nejvýše 10 snímků
   pro kontrolu nastavení. Nic se neukládá.
4. Zvýrazněte **Output** a zvolte, kam uložit (výchozí: `stacked.tif` vedle
   prvního snímku oblohy), potom klikněte na **Start**.
5. Upravte dodatečné zpracování vpravo; výsledek se hned aktualizuje.
   **Save result…** ho uloží znovu s novým nastavením.

### 13.2 Vstupní soubory

| Položka | Co nafotit |
|---|---|
| **Star images** | Snímky oblohy: stejná ohnisková vzdálenost, clona a expozice. Nejlepší je RAW; TIFF, FITS nebo XISF jen pokud jsou data stále lineární (neupravená). **Unify exposure** vyrovná snímky pořízené za různých podmínek. |
| **Base image** | Snímek, ke kterému se ostatní zarovnají. Prázdné = prostřední snímek oblohy. Jiný zvolíte tak, že zvýrazníte snímek oblohy a kliknete na **Use as base**. |
| **Noise images** | Tmavé snímky (darky): krytka na objektivu, stejné ISO a čas, pořízené těsně před snímky oblohy nebo po nich. Odstraní tepelný šum a horké pixely. |
| **Bias images** | Nejkratší čas, krytka na objektivu, stejné ISO. |
| **Vignetting images** | Flaty: rovnoměrně osvětlená bílá plocha, stejná ohnisková vzdálenost a clona. Opraví tmavší rohy a stíny prachu. |
| **Flat-dark images** | Darky k flatům: stejné ISO a čas jako vinětační snímky. |
| **Foreground image** | Nepovinné: jeden snímek země ze stejného místa stativu – za modré hodinky, s dlouhou expozicí nebo nízkým ISO. Pod oblastí oblohy nahradí (zašuměné, tmavé) složené popředí. **Match brightness** ho přizpůsobí jasu složeného popředí; **Exposure (EV)** ho odtud zesvětlí nebo ztmaví. Potřebuje oblast oblohy. |
| **Output** | `.tif` (16 bitů), `.jpg` (8 bitů) nebo `.fits` / `.xisf` (32bitová čísla s plovoucí čárkou, lineární, k dalšímu zpracování v Siril, PixInsight apod.). |

Všechny kalibrační snímky jsou nepovinné. Při vysokém ISO hodně pomůže i
několik tmavých snímků.

### 13.3 Nastavení

| Nastavení | Co dělá |
|---|---|
| **Composition** | **Align stars**: snímky se zarovnají podle hvězd a spojí. **Accumulation** je průměruje; **Select best pixels** (výchozí) vynechá odlehlé hodnoty (letadla, satelity); **Freeze ground** zarovná jen oblohu, takže obloha i popředí zůstanou ostré (potřebuje oblast oblohy); **Align only** jen zapíše zarovnané snímky. **Star trails**: bez zarovnání, obloha si ponechá nejjasnější hodnotu každého pixelu. **Trail fade**: **Fade in** (stopy začínají slabě, efekt pohybu ze Sequatoru), **Fade out** nebo **Both ends** (zúžení na obou koncích jako u komety). **Fill gaps between frames**: krátké přestávky intervalu fotoaparátu se vyplní podle změřeného pohybu hvězd, takže stopy jsou souvislé čáry, ne čárkované. |
| **Sky region** | Kde je obloha. **Boundary line**: táhněte čáru podél horizontu. **Gradient**: čára s plynulým přechodem. **Irregular mask**: oblohu malujte levým tlačítkem myši, mažte pravým; **Brush size** (posuvník nebo kolečko myši). **Detect sky** namaluje první masku podle jasu a hvězd – přibližně, proto ji zkontrolujte a opravte štětcem. **Mask feather** změkčí okraj masky, jen směrem do oblohy, takže popředí nedostane nic z oblohy (žádné svatozáře podél horizontu). **Show detected stars** zakroužkuje hvězdy, které poslední běh našel v zobrazeném snímku. |
| **Sigma clipping** | Jak přísně se odstraňují odlehlé hodnoty (dolní / horní × σ). Obvykle 2–3; nižší odstraní víc, ale nechá víc šumu. Potřebuje aspoň 4 snímky, nejlépe 10 a více. |
| **Frame rejection** | Vynechá snímky zřetelně horší než typický: mraky nebo rosa (slabší hvězdy), málo hvězd, rozmazané nebo protáhlé hvězdy (zaostření, vítr, ťuknutí do stativu). |
| **RAW conversion** | Vyvážení bílé (fotoaparát, průměr snímku, denní světlo), demozaikování (výchozí AHD; DHT nebo AAHD mají při vysokém ISO často méně barevného šumu) a **half size** (4× méně pixelů, mnohem rychlejší). |
| **Reduce dynamic noises** | Najde a odstraní horké pixely i bez tmavých snímků. |
| **Reduce distor. effects** | Opraví zkreslení objektivu, které v rozích dlouhých sérií zanechává krátké stopy: **Lens**, **Complex** (ultraširokoúhlé objektivy, refrakce u horizontu) nebo **Auto**. |
| **Merge 4 pixels** | Sečte 2×2 pixely: jasnější a méně zašuměné, poloviční šířka i výška. |
| **Time-lapse** | Složí vždy N po sobě jdoucích snímků do jednoho; vznikne očíslovaná řada snímků s nízkým šumem ve složce vedle výstupu. |

### 13.4 Zobrazení

- **Base image**: snímek, ke kterému se ostatní zarovnávají; zde se kreslí
  oblast oblohy. Kliknutím na soubor v seznamu vstupů ho zobrazíte.
- **Result**: výsledek po dodatečném zpracování. Tažením posouváte, kolečkem
  myši zvětšujete, **Fit** ukáže celý.
- **Frame report**: graf všech měření pro každý snímek, vždy vydělených
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
| **Background extraction** | Odečte hladký model pozadí oblohy (přechody od světelného znečištění nebo Měsíce). |
| **Color calibration** | Pozadí oblohy bude neutrálně šedé a průměrná hvězda bílá. |
| **Reduce colour noise** | Vyhladí jen barvy a odstraní barevné tečky ze snímků s vysokým ISO. |
| **Light pollution** | **Deep sky**: odstraní hladkou záři. **Uneven**: sleduje záři po dlaždicích (kopule světla měst na širokoúhlých snímcích). **Strength** určuje, kolik se odstraní. Nejdřív opravte vinětaci (vinětační snímky), jinak se zhorší. |
| **Enhance star light** | Zvýrazní hvězdy nad pozadím oblohy, aniž by zesílilo jeho šum. |
| **Auto brightness** | Nastaví oblohu na příjemný jas, aniž by se přepálily nejjasnější hvězdy. |
| **High dynamic range** | Jasné hvězdy a světla stlačí místo přepálení. |
| **Tone** | **sRGB curve** (jako fotoaparát), **Stretch** (černý bod, střední tóny, bílý bod; automaticky nebo ručně) nebo **Linear** (bez křivky). |
| **Color space** | sRGB nebo Adobe RGB pro výstup TIFF a JPEG. |
| **Linear (for processing)** | Vše vypne a uloží lineární 32bitový FITS: to, co očekává Siril nebo PixInsight. |
| **Use in the Stitcher** | Přidá uložený výsledek do karty Panorama stitcher jako jeden díl panoramatu (viz 13.7). |

### 13.6 Projekty

**Save project…** zapíše soubor `.stackproj` se seznamy souborů, oblastí
oblohy a všemi nastaveními. **Open project…** (nebo `panostitch
noc.stackproj`) ho znovu otevře. Soubory se hledají podle plné cesty, nebo
relativně k souboru projektu, pokud byla složka přesunuta.

### 13.7 Panoramata ze složených dílů

Panorama Mléčné dráhy s nízkým šumem: na každé pozici panoramatu pořiďte
několik snímků (pozice se překrývají), potom:

1. Složte snímky první pozice (Start) a klikněte na **Use in the Stitcher**.
2. Nahraďte snímky oblohy snímky další pozice, dejte výstupu nový název,
   Start, znovu **Use in the Stitcher**; a tak dále.
3. Na kartě Panorama stitcher spojí **Run all steps** složené díly.

Každý díl ukládejte jako `.tif`; stejné dodatečné zpracování všech dílů
udrží jejich jas a barvy stejné.

### 13.8 Tipy

- Pořiďte aspoň 10–20 snímků oblohy se stejným nastavením a hned potom
  10–20 tmavých snímků, aniž byste pohnuli fotoaparátem.
- Oblast oblohy nastavte pečlivě: stromy a budovy započtené do oblohy zarovnání
  rozmaže. **Detect sky** je rychlý začátek, ne poslední slovo.
- Po Quick preview se podívejte do Frame report: graf hned ukáže, které
  snímky jsou horší (mraky, vítr, rosa).
- Když záleží na popředí, použijte **Freeze ground**; když ne, **Select best
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
| `*.panoproj`, `*.stackproj` | Vaše soubory projektů (kamkoli je uložíte). |
| `ui.toml` ve složce nastavení | Přepínače vzhledu (viz [Spuštění programu](#2-spuštění-programu)). |
| `docs/manual_cs.md` | Tato příručka (česky). |
| `docs/manual_en.md`, `docs/manual_de.md` | Příručka v angličtině a němčině. |

Zdrojový kód (pro vlastní sestavení programu):

| Cesta | Obsah |
|---|---|
| `Cargo.toml` | Projekt a knihovny, které používá. |
| `src/image.rs`, `src/io/` | Obrazy; načítání a ukládání (JPEG, PNG, TIFF, BMP, RAW, FITS, XISF). |
| `src/stitcher/` | Spojování: body, párování, umístění, zpřesnění, projekce, prolínání. |
| `src/stacker/` | Skládání hvězd. |
| `src/gui/` | Okno: obě karty a dialogy souborů. |
| `src/bin/` | Oba programy, `panostitch` a `panostack`. |
| `tests/` | Testy: skládání na umělých nočních scénách, spojování, formáty souborů a obě karty okna. Spustíte je příkazem `cargo test --release`. |
| `examples/scripted.rs` | Použití spojování z vlastního kódu v Rustu. |
