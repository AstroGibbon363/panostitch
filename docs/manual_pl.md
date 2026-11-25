# Panorama Stitcher: podręcznik użytkownika

*Inne języki: [English](manual_en.md) · [Česky](manual_cs.md) · [Deutsch](manual_de.md) · [Español](manual_es.md) · [Français](manual_fr.md)*

Program na komputer, który łączy zachodzące na siebie zdjęcia w jedną
panoramę lub mozaikę: obrazy JPEG, TIFF, PNG, RAW z aparatu i astronomiczne
FITS. Każdy krok uruchamia się dopiero po naciśnięciu jego przycisku, a każdy
wynik można sprawdzić i poprawić przed przejściem dalej. Inne karty stackują
zdjęcia gwiazd w niebo o niskim szumie lub w ślady gwiazd
([Stackowanie gwiazd](#13-stackowanie-gwiazd)), wykańczają obrazy ([Obróbka](#16-obróbka)) i tworzą
filmy ([Timelapse](#17-timelapse)).

Elementy sterujące są w tym podręczniku **pogrubione**, tak jak widzisz je na
ekranie po polsku. Język okna zmienia się w prawym górnym rogu; ten podręcznik
jest też w programie pod **? Pomoc** (F1).

---

## Spis treści

1. [Wymagania](#1-wymagania)
2. [Uruchamianie programu](#2-uruchamianie-programu)
3. [Okno w skrócie](#3-okno-w-skrócie)
4. [Szybki start](#4-szybki-start)
5. [Krok po kroku](#5-krok-po-kroku)
6. [Poruszanie się w widoku](#6-poruszanie-się-w-widoku)
7. [Projekcje](#7-projekcje)
8. [Pliki RAW](#8-pliki-raw)
9. [Pliki FITS (astronomia)](#9-pliki-fits-astronomia)
10. [Zapisywanie](#10-zapisywanie)
11. [Rozwiązywanie problemów](#11-rozwiązywanie-problemów)
12. [Wskazówki dla dobrych panoram](#12-wskazówki-dla-dobrych-panoram)
13. [Stackowanie gwiazd](#13-stackowanie-gwiazd)
14. [Użycie bez okna](#14-użycie-bez-okna)
15. [Pliki](#15-pliki)
16. [Obróbka](#16-obróbka)
17. [Timelapse](#17-timelapse)

---

## 1. Wymagania

- **Nic nie trzeba instalować.** Program to jeden plik, `panostitch`
  (`panostitch.exe` w Windows), plus `panostack` do stackowania gwiazd z
  wiersza poleceń. W Linuksie potrzebny jest sterownik grafiki z OpenGL oraz X11
  lub Wayland, jak w każdym środowisku graficznym.
- **Pliki RAW z aparatu** nie wymagają niczego więcej: dekoder RAW jest
  wbudowany. Używane są też `dcraw_emu` z LibRaw, RawTherapee, darktable lub
  dcraw, jeśli są zainstalowane (zob. [Pliki RAW](#8-pliki-raw)).
- **Pliki FITS i XISF** nie wymagają niczego więcej.
- **Okna wyboru plików** nie wymagają niczego w Windows i macOS. W Linuksie
  używają `zenity`, `kdialog` lub `yad`, zależnie od tego, co jest
  zainstalowane (większość środowisk ma któryś). Bez nich wpisuje się ścieżki
  w pole ścieżki.

Aby zbudować program z kodu źródłowego, potrzebny jest Rust
(<https://rustup.rs>): `cargo build --release` tworzy
`target/release/panostitch` i `target/release/panostack`.

---

## 2. Uruchamianie programu

Kliknij dwukrotnie `panostitch` albo uruchom go z terminala:

```
panostitch
```

Możesz też od razu podać zdjęcia lub plik projektu do otwarcia:

```
panostitch ~/Obrazy/pano/*.NEF
panostitch moja_pano.panoproj
panostitch noc.stackproj            # otwiera się w karcie Stackowanie gwiazd
panostitch noc.tlproj               # otwiera się w karcie Timelapse
```

Uwagi:

- Program sam korzysta z rdzeni procesora. Długie zadania używają najwyżej
  85 % z nich i zostawiają 15 % pamięci wolne, żeby komputer dało się dalej
  używać.
- Okno otwiera się w sekundę lub dwie; nic nie trzeba przygotowywać.
- Jeśli okno się nie pojawi, uruchom program z terminala: tam widać komunikaty
  o błędach.
- Przełączniki wyglądu (**Ciemny**, **Nowoczesny**, **Prosty**) są zapisywane w
  `ui.toml` w folderze ustawień: `~/.config/panostitch` w Linuksie,
  `~/Library/Application Support/panostitch` w macOS,
  `%APPDATA%\panostitch` w Windows.
- **Język:** okno mówi po polsku, angielsku, czesku, niemiecku, hiszpańsku lub
  francusku; menu w prawym górnym rogu przełącza od razu (pierwsze
  uruchomienie idzie za językiem systemu).
- **Pomoc:** **? Pomoc** w prawym górnym rogu (lub F1) otwiera ten podręcznik
  w programie, ze spisem treści, wyszukiwaniem i odnośnikami; F1 otwiera
  rozdział pokazanej karty.
- **Zapamiętywane między sesjami:** karta, w której byłeś, ustawienia
  stackowania, ustawienia wideo timelapse (deflicker, ruch kamery, wynik,
  **RAW przez**) i ustawienia RAW łączenia panoram. Pliki, maski i wygląd nie
  są zapamiętywane; przechowuje je plik projektu.
- **Przeciągnij i upuść:** upuść zdjęcia lub folder na okno, by dodać je do
  pokazanej karty (Obróbka otwiera pierwszy obraz). Upuszczony plik projektu
  otwiera się w swojej karcie.
- **Ostatnie projekty** (górny pasek) pokazuje ostatnio otwarte lub zapisane
  projekty wszystkich rodzajów; kliknięcie otwiera projekt w jego karcie.
- Okna wyboru plików zaczynają w ostatnio użytym folderze.
- Po zapisaniu **Pokaż w folderze** (w raporcie lub obok **⬇ Zapisz** w
  Obróbce) otwiera folder w menedżerze plików.

---

## 3. Okno w skrócie

```
┌──────────────────────┬──────────────────────────────────────┬─────────────────────┐
│ [Open…] [Save proj.] │ [Image+keypoints] [Pair matches]     │ 3 · Projection      │
│ Images               │ [Panorama] [Zoom +] [Zoom -] [Fit]   │  projection menu    │
│  Add Remove Clear ▲▼ │ (o) overlays  [Undo] [Redo]          │  Focal, Yaw, Pitch, │
│  path box            │                                      │  Roll               │
│  image list          │                                      │ 4 · Compose         │
│  Reference           │            main view                 │  Blend, Preview size│
│  RAW via, half size  │                                      │  Exposure,Brightness│
│  Preset              │                                      │  show unlinked      │
│ 1 · Features         │                                      │  Auto crop, Uncrop  │
│                      │                                      │  Nudge ← ↑ ↓ →      │
│ 2 · Matching         │                                      │ 5 · Stretch         │
│  Pairs, Model, Pair  │                                      │  histogram, sliders │
│  pair details        │  status line (red text)              │ [Preview] [Save…]   │
└──────────────────────┴──────────────────────────────────────┴─────────────────────┘
```

(Schemat pokazuje nazwy po angielsku; w oknie są w wybranym języku.)

- **Lewa kolumna:** otwieranie i zapisywanie projektów, wybór zdjęć,
  wyszukiwanie punktów, dopasowanie zdjęć.
- **Środek:** główny widok, z przyciskami do zmiany widoku, powiększania,
  ukrywania nakładek oraz cofania i ponawiania.
- **Prawa kolumna:** projekcja, ekspozycja, mieszanie, jasność, ręczne
  rozmieszczenie, rozciąganie histogramu, podgląd i zapis.
- **Czerwony pasek stanu** (pod widokiem): co program robi, wyniki i
  komunikaty o błędach.

Przełączniki w prawym górnym rogu zmieniają wygląd: **Ciemny**, **Nowoczesny**
(bardziej zaokrąglone, przestronniejsze elementy) i **Prosty**. Tryb prosty
ukrywa szczegółowe ustawienia (wykrywanie i dopasowanie, mieszanie,
rozciąganie, dokładne przesuwanie) i zostawia to, czego potrzebuje większość
panoram: zdjęcia, ustawienie, **Wykonaj wszystkie kroki**, projekcję z
ogniskową, odchyleniem, pochyleniem i przechyłem, ekspozycję, **Wyrównaj**,
kadrowanie, podgląd i zapis. Ukryte ustawienia zachowują wartości z ustawienia.
Program pamięta przełączniki.

---

## 4. Szybki start

**Panorama fotograficzna:**

1. Zostaw włączone **RAW w połowie rozmiaru**. To szybsze i wystarcza do
   sprawdzenia wyniku.
2. Wybierz **Ustawienie** pasujące do tematu (zob. [Ustawienia](#50-ustawienia)).
3. Wpisz folder ze zdjęciami w pole ścieżki i naciśnij **Enter** albo kliknij
   **Dodaj…** i wybierz zdjęcia. **Kolejność nie ma znaczenia.**
4. Kliknij **Wykonaj wszystkie kroki** i poczekaj, aż pasek stanu pokaże
   *Gotowe*.
5. Wybierz **projekcję** po prawej: **cylindryczna** dla jednego rzędu,
   **sferyczna** dla kilku rzędów lub tego, co sięga daleko w górę lub w dół.
   Kliknij **Automatyczna ekspozycja**, jeśli jest za ciemno, i ustaw
   **Pochylenie (°)**, jeśli horyzont jest wygięty. Kliknij **Automatyczne
   kadrowanie**, by odciąć postrzępione brzegi.
6. Kliknij **⬇ Zapisz panoramę…**. Kliknij **⬇ Zapisz projekt…**, jeśli chcesz
   do niej później wrócić.

**Mozaika deep-sky z plików FITS:**

1. Wybierz ustawienie **mozaika deep-sky**.
2. Wczytaj kafelki FITS (stackowane lub pojedyncze klatki). Do podglądu są
   rozciągane automatycznie.
3. Kliknij **Wykonaj wszystkie kroki**, potem **Automatyczne rozciągnięcie**,
   by dopasować rozciąganie do całej mozaiki, i popraw je według uznania.
4. Zapisz jako **`.fits`**, by zachować dane liniowe do dalszej obróbki, albo
   jako **`.tif`**, by dostać rozciągnięty obraz, który widzisz.

---

## 5. Krok po kroku

**Wpisywanie wartości:** każdy suwak ma obok pole liczbowe. Kliknij pole,
wpisz wartość i naciśnij **Enter**. Wartość dopasuje się do kroku i zakresu
suwaka.

### 5.0 Ustawienia

Menu **Ustawienie** (lewa kolumna, pod **RAW przez**) ustawia naraz wszystkie
parametry dopasowania i składania dla rodzaju tematu. Każdą wartość można
potem zmienić.

| Ustawienie | Do czego | Główne parametry |
|---|---|---|
| **krajobraz dzienny** | panoramy dzienne, aparat obracany w miejscu | lekkie odszumianie, homografia, cylindryczna |
| **astro / krajobraz nocny** | nocne niebo, Droga Mleczna nad krajobrazem | mocne odszumianie, luźniejsze dopasowanie, sferyczna, +1 EV (potem **Automatyczna ekspozycja**), wyższy próg narożników (mniej fałszywych narożników w szumie) |
| **mozaika deep-sky** | mozaiki pól gwiazdowych z teleskopu, np. FITS | małe odszumianie (gwiazdy są malutkie), model podobieństwa (przesunięcie + obrót pola), bez dopracowania, prostoliniowa; użyj **Automatyczne rozciągnięcie** |
| **makro / mikroskop** | szkiełko lub stolik przesuwany pod nieruchomym aparatem | model przesunięcia (klatki tylko się przesuwają), bez dopracowania, prostoliniowa |
| **skan / płaski dokument** | mapy, obrazy, dokumenty fotografowane częściami | model afiniczny, bez dopracowania, prostoliniowa |

### 5.1 Zdjęcia

| Element | Co robi |
|---|---|
| **Dodaj…** | Otwiera okno wyboru plików. Można wybrać kilka plików (Ctrl/Shift-klik). |
| **Pole ścieżki** | Wpisz ścieżkę pliku lub **folderu** i naciśnij Enter. Folder wczytuje wszystkie zdjęcia, posortowane według nazwy. Pliki o nazwie `panorama…` (wcześniejsze wyniki) są pomijane. |
| **Lista zdjęć** | Kliknij zdjęcie, by je zaznaczyć i pokazać. **Ctrl-klik** dodaje lub usuwa zdjęcie z zaznaczenia, **Shift-klik** zaznacza zakres, a **Ctrl+A** (z myszą nad listą) zaznacza wszystko. Przewijaj kółkiem myszy. |
| **Usuń** | Usuwa zaznaczone zdjęcia. Klawisz **Delete** robi to samo, gdy mysz jest nad listą. |
| **Wyczyść wszystko** | Usuwa wszystkie zdjęcia. |
| **▲ / ▼** | Przesuwa zaznaczone zdjęcia w górę lub w dół listy. Kolejność ma znaczenie tylko w trybach par *po kolei* i *siatka*. |
| **Odniesienie** | Zdjęcie, wokół którego rozmieszczane są pozostałe; pozostaje niezniekształcone. **auto (środek)** wybiera zdjęcie ze środka grupy, co zwykle daje najrówniejszy wynik. |
| **RAW przez** | Którego konwertera RAW użyć; zob. [Pliki RAW](#8-pliki-raw). Dotyczy zdjęć dodanych później. |
| **RAW w połowie rozmiaru** | Dekoduje pliki RAW i pliki FITS z kolorowych kamer w połowie rozdzielczości: około 4× szybciej i ¼ pamięci. Dotyczy zdjęć dodanych później. Pliki RAW są odczytywane ponownie w pełnym rozmiarze, gdy zapisujesz z **Rozmiarem wyniku** powyżej 50 %. |

Lista oznacza każdy wpis:

| Znak | Znaczenie |
|---|---|
| niebieska linia | zaznaczone; ciemniejsze to bieżące zdjęcie (do oglądania, jasności i przesuwania) |
| `R` | zdjęcie odniesienia |
| `4345kp` | liczba znalezionych punktów |
| `UNLINKED` | zdjęcia nie udało się połączyć z panoramą (zob. [5.3](#53-krok-2-dopasowanie)) |

Wyniki dopasowania są zachowywane, gdy zmieniasz kolejność lub usuwasz inne
zdjęcia, więc nie trzeba dopasowywać od nowa.

### 5.2 Krok 1: punkty

Punkty charakterystyczne to wyraźne miejsca, takie jak narożniki, krawędzie i
gwiazdy, które da się rozpoznać na dwóch zachodzących zdjęciach.

| Suwak | Domyślnie | Znaczenie |
|---|---|---|
| **Rozmiar wykrywania (px)** | 2000 | Przed szukaniem zdjęcie jest zmniejszane do tego rozmiaru (dłuższy bok). Większy jest dokładniejszy, ale wolniejszy. |
| **Maks. punktów** | 5000 | Górny limit na zdjęcie. Punkty są rozkładane równo po całym zdjęciu. |
| **Próg narożników** | 0.001 | Minimalna siła narożnika. Niżej znajduje słabsze narożniki (przydatne dla nieba lub mgły); wyżej zostawia tylko mocne. |
| **Odszumianie (σ px)** | 1.5 | Rozmywa szum matrycy przed szukaniem. Zwiększ (2–3) dla **zdjęć nocnych** lub zaszumionych zdjęć z wysokim ISO; zmniejsz (0,5–1) dla pól gwiazd i czystych zdjęć dziennych z drobnymi szczegółami. |

Kliknij **Wykryj punkty**. Widok pokazuje bieżące zdjęcie z punktami jako
**żółte kółka**. Powinny być rozłożone po tych częściach zdjęcia, które
zachodzą na inne zdjęcia. Ponownie przetwarzane są tylko zdjęcia, których
ustawienia się zmieniły. Ciemne i liniowe zdjęcia (nocne, FITS) są obsługiwane:
wykrywanie samo dostosowuje kontrast.

### 5.3 Krok 2: dopasowanie

Dopasowanie znajduje te same punkty na dwóch zdjęciach. Potem **RANSAC**
zostawia tylko dopasowania zgodne z jednym modelem geometrycznym (*inliery*).

**Które pary są sprawdzane (menu Pary):**

| Pary | Kiedy |
|---|---|
| **wszystkie pary** | Domyślnie. Każde zdjęcie jest sprawdzane z każdym innym, więc **kolejność nie ma znaczenia**, a kilka rzędów działa samo. Dłużej trwa przy wielu zdjęciach (74 zdjęcia ≈ 35 s). |
| **po kolei** | Jeden rząd, zdjęcia w kolejności robienia. Każde zdjęcie jest dopasowywane tylko z następnym na liście. Najszybsze. |
| **siatka** | Kilka rzędów w znanej kolejności: **Kolumny siatki** zdjęć na rząd, rząd po rzędzie. Sprawdzani są sąsiedzi po prawej, poniżej i po przekątnej. Włącz **wąż**, jeśli co drugi rząd był robiony w przeciwnym kierunku. |

**Ustawienia:**

| Element | Domyślnie | Znaczenie |
|---|---|---|
| **Kolumny siatki** | 5 | Zdjęć na rząd (tylko tryb siatki). |
| **Test proporcji** | 0.8 | O ile najlepsze dopasowanie musi być lepsze od drugiego. Niżej jest surowiej (mniej, ale pewniejszych dopasowań); wyżej daje więcej dopasowań. |
| **Maks. odległość** | 0.35 | Jak bardzo dwa punkty mogą się różnić i nadal pasować. |
| **Tol. RANSAC (px)** | 5.0 | Jak daleko (w pikselach) dopasowanie może być od modelu i nadal liczyć się jako inlier. |
| **Min. inlierów** | 20 | Najmniejsza liczba inlierów, by para liczyła się jako prawdziwa zakładka. |
| **Model** | homografia | Jak jedno zdjęcie przechodzi na drugie, zob. niżej. |
| **dopracuj** | wł. | Globalne dopracowanie, zob. niżej. |

**Kiedy para liczy się jako prawdziwa zakładka:** potrzebuje co najmniej
*Min. inlierów* inlierów, wyraźna większość jej dopasowań musi się zgadzać, a
przekształcenie musi być wiarygodne (nie może zginać ani skrajnie rozciągać
zdjęcia). To odsiewa fałszywe dopasowania między zdjęciami, które w ogóle na
siebie nie zachodzą.

**Modele:**

| Model | Kiedy |
|---|---|
| **homografia** | Zwykłe panoramy: aparat obracany w miejscu. Domyślny. |
| **afiniczna** | Skany lub zdjęcia płaskiego obiektu z daleka. |
| **podobieństwo** | Tylko przesunięcie, obrót i jednolite skalowanie, np. mozaiki z teleskopu z obrotem pola. Stabilny przy niewielu dopasowaniach. |
| **przesunięcie** | Tylko przesunięcie, np. skaner lub stolik mikroskopu. Najodporniejszy, najmniej elastyczny. |

**Rozmieszczenie i globalne dopracowanie (dopracuj):** zdjęcia są łączone przez
ich najsilniejsze pary, zaczynając od zdjęcia odniesienia. Gdy **dopracuj**
jest włączone (tylko model homografii), wszystkie pary są potem rozwiązywane
razem jako jeden obracający się aparat. Małe błędy rozkładają się równo zamiast
się sumować, a pętle między rzędami się zamykają; to usuwa większość duchów.
Dopracowanie szacuje też beczkowatą dystorsję obiektywu (silną w
szerokokątnych) i koryguje ją przy renderowaniu. Pary, które nie pasują do
wspólnego rozwiązania, są **odrzucane automatycznie**, bo to zwykle fałszywe
dopasowania lub coś, co się poruszyło. Pasek stanu podaje, ile odrzucono, i
**typowy błąd pary** (1–3 px to dobrze).

**Nocne niebo: dopasowanie po gwiazdach.** Punkty narożne rzadko dopasowują
klatki, na których jest tylko niebo: każda gwiazda wygląda jak każda inna. Gdy
punkty pary zawiodą, porównywane są więc jej **gwiazdy**: trójkąty sąsiednich
gwiazd, według kątów między nimi. Te kąty nie zmieniają się, gdy obraca się
aparat ani gdy obraca się niebo, więc dopasowują się nawet klatki zrobione w
odstępie minut. Takie pary mają w menu **Para** dopisek *(gwiazdy)*, a pasek
stanu je liczy. Akceptowane są tylko pary z wystarczającą liczbą gwiazd w
części wspólnej, z których spora część pasuje. Gdy aparaty celują daleko od
siebie (rząd w górę i rząd w dół), dalekie rogi zdjęcia leżą za drugim
aparatem; wtedy oceniana jest tylko część pokryta dopasowaniami.

**Zoom, który się przesunął.** Skierowany w górę lub w dół pierścień zoomu może
się obrócić pod własnym ciężarem między ujęciami. W plikach RAW ogniskowa
każdego zdjęcia jest znana: punkty są szukane we wszystkich w tej samej skali,
a dopracowanie dopasowuje każdą ogniskową osobno. Pasek stanu pisze wtedy *Zoom
zmieniał się między zdjęciami (15–17 mm)*.

**Obracające się niebo.** Niebo obraca się o 15° na godzinę, więc rzędy
zrobione w odstępie dziesięciu minut widzą je przesunięte o 2,5°. W plikach RAW
(zapisują czas każdego ujęcia) dopracowanie mierzy ten obrót z par gwiazd
razem z parami na ziemi, więc aparaty są rozmieszczone poprawnie. Pasek stanu
pisze *niebo obracało się między zdjęciami o 14,9°/godz.*: blisko 15 znaczy, że
się udało. Przy renderowaniu gwiazdy wszystkich zdjęć są pokazane tak, jak były
w jednej chwili (w środku sesji), a ziemia i poświata nieba zostają na swoim
miejscu; przy drzewach i innych sylwetkach każde zdjęcie zostaje takie, jak
zostało zrobione.

**Przyciski:**

- **Dopasuj**: dopasowuje pary wybrane w menu Pary (równolegle), potem
  rozmieszcza i dopracowuje.
- **Dopasuj parę ponownie**: dopasowuje tylko pokazaną parę, z bieżącymi
  ustawieniami.
- **Odrzuć parę**: przestaje używać pokazanej pary (dla dopasowania, które
  widocznie jest złe); rozmieszczenie jest liczone od nowa bez niej.
- **Wykonaj wszystkie kroki**: punkty, dopasowanie, rozmieszczenie i podgląd
  za jednym razem.

Widok **Dopasowania par** pokazuje dwa zdjęcia obok siebie:

- **zielone linie**: inliery (poprawne dopasowania)
- **czerwone linie**: odstające. Ukryj je przełącznikiem **odstające**.

Menu **Para** wymienia zaakceptowane pary z liczbą inlierów. Pary oznaczone
*odrzucona* nie przeszły któregoś sprawdzenia; tekst pod menu mówi dlaczego.

**Jak wygląda dobry wynik:** dziesiątki zielonych linii lub więcej, wszystkie
opisujące to samo przesunięcie, i średni błąd 1–2 px.

**Niepołączone zdjęcia:** zdjęcie bez żadnej zaakceptowanej pary z resztą jest
oznaczone `UNLINKED` na liście i **pominięte w panoramie**, żeby jej nie
zepsuło. Włącz **pokaż niepołączone zdjęcia** (prawa kolumna), by pokazać takie
zdjęcia w rzędzie pod panoramą i rozmieścić je ręcznie przez
**Ctrl+przeciąganie**.

### 5.4 Krok 3: projekcja

Zob. [Projekcje](#7-projekcje), jak wygląda każda z nich.

| Element | Znaczenie |
|---|---|
| **Menu projekcji** | 14 projekcji, zob. [Projekcje](#7-projekcje) |
| **Ogniskowa (× auto)** | Ogniskowa jest szacowana z dopasowań. Ten suwak ją skaluje. Jeśli panorama wygląda na ściśniętą lub rozciągniętą w poziomie, spróbuj wartości między 0,8 a 1,2. |
| **Odchylenie (°)** | Obraca całą panoramę w lewo lub w prawo: wybiera, który kierunek jest w środku. Przydatne w panoramach 360° (np. by umieścić centrum Drogi Mlecznej pośrodku zamiast przy krawędzi). |
| **Pochylenie (°)** | Pochyla całą panoramę w górę lub w dół. Użyj, by **wyprostować wygięty horyzont** (np. gdy aparat był skierowany w dół). |
| **Przechył (°)** | Obraca całą panoramę. Użyj, by wypoziomować przekrzywiony horyzont. |
| *Dokładne ustawianie* | Odchylenie, pochylenie i przechył mają suwak, pole liczbowe i **◀ ▶**. Suwaki pochylenia i przechyłu są dokładne w pobliżu 0° (gdzie zwykle są poprawki) i zgrubne dalej. Przeciągaj pole liczbowe powoli, by zmieniać o 0,02° na piksel, albo kliknij dwukrotnie i wpisz. **◀ ▶** zmieniają o 0,1° (z Shiftem o 1°). |
| **Resetuj ogniskową / odchylenie / pochylenie / przechył** | Przywraca te cztery suwaki do wartości domyślnych. |

Tytuł widoku pokazuje rozmiar panoramy, ogniskową w pikselach, pole widzenia
zdjęcia odniesienia i typowy błąd wyrównania.

Te elementy od razu aktualizują podgląd (gdy podgląd już istnieje).

### 5.5 Krok 4: składanie

| Element | Znaczenie |
|---|---|
| **Mieszanie** | **inteligentne** (domyślnie): mieszanie wielopasmowe z usuwaniem duchów. Drobne szczegóły pochodzą z jednego zdjęcia w każdym miejscu, więc nie ma podwójnych obrazów ludzi, świateł ani bliskich obiektów. Różnice jasności są mieszane w szerokiej strefie, więc nie ma twardych łączeń. Tam, gdzie zachodzące zdjęcia się nie zgadzają (coś się poruszyło), cały ten obszar pochodzi z jednego zdjęcia. **przenikanie**: zwykłe łagodne mieszanie; pokazuje duchy, gdzie coś się poruszyło. **szew**: każdy piksel z najbardziej centralnego zdjęcia; bez duchów, ale z twardymi łączeniami. **średnia**: zwykła średnia; głównie do sprawdzenia wyrównania. |
| **Rozmiar podglądu (px)** | Rozmiar podglądu (dłuższy bok). Większy jest ostrzejszy, ale wolniejszy. Przy powiększaniu szczegóły i tak są renderowane (zob. [Poruszanie się w widoku](#6-poruszanie-się-w-widoku)). |
| **Ekspozycja (EV)** | Rozjaśnia lub przyciemnia całą panoramę, od −3 do +8 EV (+1 EV = dwa razy jaśniej). Ciemne panoramy astro często potrzebują +1,5 do +4. |
| **Jasność (zazn.)** | Współczynnik jasności **zaznaczonych** zdjęć. |
| **pokaż niepołączone zdjęcia** | Pokazuje też zdjęcia, których nie udało się połączyć, w rzędzie pod panoramą, do ręcznego rozmieszczenia. |
| **Automatyczna ekspozycja** | Ustawia **Ekspozycję** z bieżącego podglądu: typowa jasność na średnią szarość, bez przepalania więcej niż najjaśniejszych 0,5 % (gwiazdy, lampy). |
| **Wyrównaj** | Oblicza współczynniki jasności dla każdego zdjęcia, by zakładki do siebie pasowały. |
| **Resetuj przesunięcia** | Cofa wszystkie ręczne przesunięcia. |
| **Automatyczne kadrowanie** | Przycina panoramę do największego prostokąta całkowicie pokrytego zdjęciami, więc znikają czarne, postrzępione brzegi. Podgląd pokazuje wtedy przycięty wynik, a **⬇ Zapisz panoramę…** go zapisuje. Pasek stanu podaje, ile panoramy zostaje. Najpierw wyprostuj horyzont (pochylenie, przechył); wygięta panorama zostawia tylko wąski prostokąt. |
| **Usuń kadrowanie** | Usuwa kadrowanie. Kadrowanie znika też samo, gdy zmienisz coś, co przesuwa zdjęcia (projekcja, ogniskowa, odchylenie, pochylenie, przechył, przesunięcia, dopasowanie). |
| **✂ Kadruj** (nad widokiem) | Zaznacz część panoramy, która zostaje: przeciągnij ramkę, dopasuj ją krawędziami lub rogami, przeciągnij wewnątrz, by ją przesunąć, i kliknij **✂ Kadrowanie: gotowe** (Esc anuluje). Liczona i zapisywana jest tylko ta część: duża panorama potrzebuje mniej pamięci, a do samego kadrowania nie trzeba karty Obróbka. Usuwa się je jak automatyczne kadrowanie; znika przy zmianie geometrii. |
| **⬇ Zapisz i obrób…** (nad widokiem) | Zapisuje panoramę, a potem otwiera ją w karcie Obróbka. |
| **Przesuń zaznaczone ← ↑ ↓ →** | Przesuwa bieżące zdjęcie o jeden piksel podglądu (zob. niżej). |

W widoku **Panorama** rysowany i numerowany jest obrys każdego zdjęcia:

- **pomarańczowy**: bieżące zdjęcie
- **jasnoniebieski**: inne zaznaczone zdjęcia
- **cyjan**: zdjęcie odniesienia
- **biały**: pozostałe zdjęcia
- **żółte przerywane linie**: połączenia użyte do rozmieszczenia zdjęć

Wyłącz je przyciskiem **nakładki** na pasku narzędzi, by zobaczyć samą
panoramę.

**Kliknij wewnątrz obrysu**, by zaznaczyć to zdjęcie (działa też z wyłączonymi
nakładkami).

**Ręczne przesuwanie zdjęć:** przytrzymaj **Ctrl** i przeciągnij zdjęcie albo
użyj strzałek lub przycisków przesuwania. Zdjęcie przesuwa się razem ze
zdjęciami do niego dołączonymi dalej od odniesienia (wzdłuż żółtych połączeń).
Zdjęcie odniesienia się nie rusza; do przesunięcia wszystkiego użyj pochylenia
i przechyłu.

### 5.6 Krok 5: rozciąganie

Zdjęcia astronomiczne (i ciemne wywołania RAW) są zwykle **liniowe**:
interesujące szczegóły leżą w kilku najciemniejszych procentach zakresu
jasności, a obraz wygląda na prawie czarny. Rozciąganie histogramu czyni je
widocznymi.

| Element | Znaczenie |
|---|---|
| **Histogram** | Rozkład jasności podglądu panoramy (w widoku Panorama) lub bieżącego zdjęcia (w pozostałych widokach), w skali logarytmicznej. **Niebieska linia** oznacza punkt czerni, **biała linia** punkt bieli, a **pomarańczowa krzywa** pokazuje, jak jasność wejściowa (od lewej do prawej) przechodzi w wyjściową (od dołu do góry). |
| **Punkt czerni** | Wszystko o tej jasności lub ciemniejsze staje się czarne. Ustaw go tuż pod tłem nieba (lewa krawędź głównego szczytu histogramu). |
| **Półtony** | Rozjaśnia (mniejsze wartości) lub przyciemnia (większe wartości) półtony bez obcinania. 0,5 jest neutralne. Zdjęcia deep-sky zwykle potrzebują 0,005–0,05. |
| **Punkt bieli** | Wszystko o tej jasności lub jaśniejsze staje się białe. |
| **Automatyczne rozciągnięcie** | Ustawia wszystkie trzy z bieżącego histogramu: punkt czerni tuż pod tłem nieba, półtony tak, by tło wypadło na około 25 % szarości (ten sam pomysł co automatyczne rozciąganie ekranowe w PixInsight). |
| **Resetuj rozciąganie** | Powrót do braku rozciągania (0 / 0,5 / 1). |

- Rozciąganie zmienia tylko sposób **pokazywania** zdjęć i zapisu panoramy jako
  TIFF, PNG lub JPEG. Dopasowanie zawsze działa na oryginalnych danych.
- **Pliki FITS** dostają przy wczytaniu automatyczne rozciągnięcie do
  podglądu, liczone ze wszystkich wczytanych obrazów FITS razem.
- Zmiana rozciągania jest natychmiastowa: nic nie trzeba przeliczać.
- **Ekspozycja** jest stosowana przed rozciąganiem.

**Tryby wyświetlania (tylko widok).** **Wyświetlanie** nad widokiem pokazuje
dane jak Siril: *Liniowy*, *Logarytm*, *Pierwiastek*, *Kwadrat*, *Asinh*,
*AutoStretch* lub *Wyrównanie histogramu*, z suwakami **Low** i **High**:
wartości poniżej Low są czarne, powyżej High białe (dla słabych danych
liniowych spróbuj High około 0,01–0,1; ↺ je przywraca). *Jak przetworzono*
pokazuje rozciągnięcie z tego panelu. Tryb wyświetlania nigdy nie zmienia tego,
co się zapisuje. Wynik stackowania gwiazd i karta Obróbka mają ten sam element.

### 5.7 Cofanie, ponawianie i projekty

**Cofanie / ponawianie:** **↶ Cofnij** i **↷ Ponów** na pasku narzędzi albo
**Ctrl+Z** i **Ctrl+Y** (także **Ctrl+Shift+Z**). Pasek stanu mówi, co cofnięto,
np. *„Cofnięto: Pochylenie (°) 0 → 10”*.

- Można cofnąć: każdy suwak, menu i przełącznik ustawień; dodawanie, usuwanie
  i zmianę kolejności zdjęć; wyniki dopasowania (Dopasuj, Dopasuj ponownie,
  Odrzuć); zdjęcie odniesienia; ręczne przesunięcia; jasność; automatyczne
  kadrowanie.
- Przeciągnięcie suwaka to **jeden** krok, niezależnie od tego, jak długo
  przeciągasz. Zmiany dwóch różnych elementów to zawsze dwa kroki.
- Usunięte zdjęcia są trzymane w pamięci do cofnięcia, więc wracają od razu,
  razem z wynikami dopasowania.
- Pamiętanych jest ostatnie 100 kroków. Nie da się cofnąć: tego, co dotyczy
  tylko widoku (powiększenie, który widok jest pokazany, nakładki), **RAW
  przez** i **RAW w połowie rozmiaru** oraz zapisu plików.

**Projekty:** **⬇ Zapisz projekt…** (na górze lewej kolumny) zapisuje plik
projektu z rozszerzeniem **`.panoproj`**. **⇪ Otwórz projekt…** wczytuje go z
powrotem. Można też wpisać ścieżkę pliku `.panoproj` w pole ścieżki i nacisnąć
Enter albo uruchomić z nim program: `panostitch moja_pano.panoproj`.

Projekt przechowuje:

- zdjęcia (pełną ścieżkę i ścieżkę względną wobec pliku projektu)
- wszystkie ustawienia: suwaki, menu, przełączniki, w tym rozciąganie
- wyniki dopasowania, więc nic nie trzeba dopasowywać od nowa
- ręczne przesunięcia, współczynniki jasności, zdjęcie odniesienia i
  kadrowanie
- **RAW przez** i **RAW w połowie rozmiaru**, żeby zdjęcia wczytywały się tak
  samo

**Nie** zawiera samych zdjęć; są wczytywane z ich plików. Jeśli przeniesiesz
zdjęcia razem z plikiem projektu (np. cały folder), nadal zostaną znalezione
przez ścieżkę względną. Brakujące pliki są zgłaszane i pomijane.

Plik to czytelny tekst (format TOML). Jego rozmiar to głównie zapisane punkty
dopasowań: około 50 kB na zdjęcie.

---

## 6. Poruszanie się w widoku

| Czynność | Mysz | Klawiatura | Przycisk |
|---|---|---|---|
| Przesuwanie | przeciąganie **lewym** przyciskiem | strzałki (widoki Obraz i Para) | |
| Powiększenie | kółko w górę | `+` | **Powiększ +** |
| Pomniejszenie | kółko w dół | `-` | **Pomniejsz -** |
| Reset powiększenia | | `F`, `0` lub `Home` | **Dopasuj** |
| Zaznaczenie zdjęcia (widok Panorama) | klik wewnątrz jego obrysu | | |
| Pokaż / ukryj obrysy, numery, połączenia | | | przełącznik **nakładki** |
| Przesunięcie zdjęcia (widok Panorama) | **Ctrl + przeciąganie** | strzałki (`Shift` = 10 kroków) | **← ↑ ↓ →** |
| Cofnij | | `Ctrl+Z` | **↶ Cofnij** |
| Ponów | | `Ctrl+Y` lub `Ctrl+Shift+Z` | **↷ Ponów** |

Przyciski **Obraz + punkty**, **Dopasowania par** i **Panorama** przełączają
między trzema widokami.

**Inne skróty** (najedź na przycisk, by zobaczyć jego skrót):

| Klawisze | Łączenie panoram | Stackowanie gwiazd |
|---|---|---|
| `Ctrl+Enter` | **▶ Wykonaj wszystkie kroki** | **▶ Start** |
| `P` / `Ctrl+P` | `P`: **◐ Podgląd panoramy** | `Ctrl+P`: **◐ Szybki podgląd** |
| `Ctrl+S` | zapisz projekt (od razu do otwartego pliku; za pierwszym razem pyta o nazwę) | tak samo |
| `Ctrl+Shift+S` | **⬇ Zapisz panoramę…** | |
| `Ctrl+O` | **⇪ Otwórz projekt…** | tak samo |
| `Esc` | | anuluj trwające stackowanie |
| `F` | dopasuj obraz do widoku | tak samo |
| `F1` | ten podręcznik, w rozdziale karty | tak samo |

W macOS używaj `Cmd` zamiast `Ctrl`.

**Zamknięcie okna** najpierw pyta, jeśli trwa stackowanie albo praca w karcie
nie jest zapisana w pliku projektu. Możesz wtedy zapisać projekt, zamknąć mimo
to albo anulować.

**Ostre powiększenie:** podgląd to pomniejszony obraz. Gdy powiększysz
panoramę i chwilę poczekasz, widoczny obszar jest renderowany ponownie w
rozdzielczości ekranu z oryginalnych zdjęć, by można było sprawdzić drobne
szczegóły i łączenia.

---

## 7. Projekcje

Panorama obejmuje więcej, niż może pokazać płaskie zdjęcie, więc program musi
*rzutować* widok na płaski obraz. Każda projekcja coś zniekształca:

| Projekcja | Wygląda jak | Najlepsza do | Ograniczenia |
|---|---|---|---|
| **prostoliniowa** | zwykłe zdjęcie: proste zostają proste | wąskie panoramy (do ~100°), architektura, mozaiki z teleskopu | brzegi skrajnie się rozciągają, gdy widok się poszerza |
| **cylindryczna** | poziome proste się wyginają, pionowe zostają pionowe | większość poziomych panoram | nie do wysokich widoków (góra i dół się rozciągają) |
| **sferyczna** (równoprostokątna) | jak cylindryczna, z mniejszym rozciąganiem w pionie | bardzo szerokie lub 360° panoramy, przeglądarki panoram | proste się wyginają |
| **Mercator** | pomiędzy cylindryczną a sferyczną | szerokie panoramy z pewną wysokością | rozciąga przy górze i dole |
| **stereograficzna** | jak rybie oko, zachowuje okrągłe kształty | bardzo szerokie pola widzenia, ujęcia kreatywne | silna krzywizna |
| **cylindryczna poprzeczna** | cylindryczna położona na boku: pionowe linie się wyginają, poziome zostają proste | **pionowe panoramy** (od horyzontu do zenitu), stojący łuk Drogi Mlecznej | nie do szerokich poziomych ujęć (zawijają się przez górną i dolną krawędź) |
| **Mercator poprzeczny** | jak cylindryczna poprzeczna, mniej rozciągnięta na boki | pionowe panoramy z pewną szerokością | jak wyżej |
| **rybie oko** (równoodległościowe) | okrągły obraz, odległość od środka proporcjonalna do kąta | **widoki całego nieba**: przechyl Pochyleniem tak, by zenit był w środku | ściśnięte brzegi |
| **rybie oko równopowierzchniowe** | okrągły obraz zachowujący proporcje powierzchni | widoki całego nieba, tak jak wygląda większość rybich oczu | brzegi jeszcze bardziej ściśnięte |
| **mała planeta** | ziemia w środku, niebo wokół jako pierścień | panoramy 360° z ziemią, kreatywnie | wymaga panoramy sięgającej daleko w dół |
| **tunel** | niebo w środku, horyzont wokół | nocne niebo 360°, ślady gwiazd, drzewa od dołu | wymaga panoramy sięgającej daleko w górę |
| **ortograficzna** | kula ziemska widziana z zewnątrz | kreatywnie; pokazuje połowę sfery | brzegi bardzo ściśnięte |
| **Hammer** (Hammer–Aitoff) | owalna mapa całej sfery z poprawnymi proporcjami powierzchni | panoramy pełnej sfery w jednym obrazie, mapy całego nieba | kształty zniekształcone przy brzegach |
| **Mollweide** | owalna mapa równopowierzchniowa, bardziej płaska przy biegunach | jak Hammer; klasyczna mapa całego nieba w astronomii | kształty zniekształcone przy brzegach |

Jeśli horyzont wygląda jak uśmiech lub grymas, ustaw **Pochylenie (°)**. Jeśli
jest przekrzywiony, ustaw **Przechył (°)**. **Odchylenie (°)** obraca widok na
boki. W widokach rybiego oka, małej planety i tunelu pochylenie decyduje, co
jest w środku.

---

## 8. Pliki RAW

Obsługiwane: **Canon CR2 i CR3, Nikon NEF i NRW, Sony ARW i SRF, DNG, Fuji
RAF, Olympus ORF, Panasonic RW2, Pentax PEF, Samsung SRW** i inne. Dokładna
obsługa aparatów zależy od dekodera; wbudowany zna większość aparatów z
ostatnich dwóch dekad, LibRaw (jeśli zainstalowany) około 1250.

**RAW przez** wybiera dekoder:

| Opcja | Dekoder | Charakter |
|---|---|---|
| **auto** | pierwszy działający, w kolejności poniżej; najpierw RawTherapee, jeśli zdjęcie ma plik `.pp3` | zalecane |
| **built-in** | własny dekoder programu | zawsze dostępny, szybki; neutralne wywołanie z kolorami i balansem bieli z aparatu i bez automatycznego rozjaśniania, więc wszystkie klatki zachowują tę samą ekspozycję |
| **libraw** | `dcraw_emu` (LibRaw) | podobny do wbudowanego; dla aparatów, których wbudowany nie zna |
| **rawtherapee** | `rawtherapee-cli` | **używa twoich edycji z RawTherapee** zapisanych w `zdjecie.NEF.pp3`; w przeciwnym razie domyślnego wyglądu RawTherapee |
| **darktable** | `darktable-cli` | domyślne wywołanie darktable (z korekcją obiektywu, jeśli jest); najwolniejszy |
| **dcraw** | `dcraw` | awaryjny; nie czyta CR3 |

Tylko wbudowany jest zawsze dostępny; pozostałe pojawiają się, gdy dany
program jest zainstalowany na komputerze. Stackowanie gwiazd potrzebuje danych
liniowych, więc używa tylko built-in, libraw lub dcraw.

Zewnętrzne dekodery zapisują 16-bitowy obraz w folderze tymczasowym, który
potem jest usuwany. Nic nie jest zapisywane obok twoich zdjęć.

**Wskazówka:** dla spójnych kolorów w całej panoramie używaj tych samych edycji
(tego samego profilu `.pp3`) dla wszystkich klatek albo użyj **built-in** i
popraw kolory później.

---

## 9. Pliki FITS (astronomia)

Pliki kończące się na **`.fits`, `.fit` lub `.fts`** są czytane bezpośrednio,
bez dodatkowego oprogramowania. Obsługiwane:

| Zawartość | Przykłady | Jak są czytane |
|---|---|---|
| **Liczby całkowite 8, 16, 32 i 64 bity, zmiennoprzecinkowe 32 i 64 bity** | zapisy z kamer, wyniki stackowania | skalowane do 0…1 (16 bitów bez znaku: 0…65535) |
| **Kolor (3 płaszczyzny)** | stackowane wyniki z Sirila, PixInsight, DSS | jako RGB |
| **Mono** | kamery monochromatyczne, pojedyncze filtry | jako szarość |
| **Surowe dane z kamery kolorowej (Bayer)** | klatki z ZWO, ToupTek, QHY… z nagłówkiem `BAYERPAT` (RGGB, BGGR, GRBG, GBRG) | debayeryzowane; z **RAW w połowie rozmiaru** jeden piksel koloru na komórkę 2×2 (szybko, czysto), w przeciwnym razie interpolowane w pełnym rozmiarze |
| **Kolejność wierszy** | `ROWORDER` = `TOP-DOWN` lub `BOTTOM-UP` (domyślne w FITS) | obraz jest pokazany właściwą stroną do góry |

Nieobsługiwane: **pliki skompresowane kafelkami (`.fz`)**. Najpierw je
rozpakuj, np. przez `funpack`. Dodatkowe części pliku (tabele, profile ICC) są
pomijane; używany jest pierwszy obraz.

**Uwagi do mozaik deep-sky:**

- Użyj ustawienia **mozaika deep-sky**. Kafelki z teleskopu tylko się
  przesuwają i obracają (obrót pola), co dokładnie opisuje model
  *podobieństwa*.
- **Stackowane, skalibrowane kafelki** łączą się najlepiej. Pojedyncze krótkie
  klatki mają mało widocznych gwiazd i dużo szumu.
- Surowe klatki z kamer kolorowych nie mają balansu bieli, więc wyglądają na
  zielone; rozciąganie nie zmienia balansu kolorów.
- Zapisz jako **FITS**, by kontynuować obróbkę (ekstrakcja tła, kalibracja
  kolorów, końcowe rozciąganie) w Sirilu lub PixInsight. Wynik FITS zachowuje
  wartości liniowe.

---

## 10. Zapisywanie

Kliknij **⬇ Zapisz panoramę…** i wybierz nazwę pliku. Typ pliku wynika z
rozszerzenia:

| Rozszerzenie | Wynik |
|---|---|
| `.tif` / `.tiff` | 16 bitów na kanał, najlepszy do dalszej edycji; zapisany **tak, jak widać** (z ekspozycją i rozciąganiem) |
| `.png` | 16 bitów na kanał; tak, jak widać |
| `.jpg` | 8 bitów na kanał, mniejszy plik; tak, jak widać |
| `.fits` / `.fit` / `.fts` | 32-bitowe liczby zmiennoprzecinkowe, trzy płaszczyzny kolorów; **liniowo** (z ekspozycją i współczynnikami jasności, ale **bez rozciągania** i obcinania), dla programów astronomicznych |

Gdy wszystkie wczytane zdjęcia to FITS, proponowana nazwa pliku kończy się na
`.fits`.

**Rozmiar wyniku** (nad przyciskiem zapisu) ustala wielkość zapisanej panoramy,
w % pełnej rozdzielczości aparatu; wiersz pod nim pokazuje rozmiar w pikselach
(po podglądzie). Przyciski ¼, ½, pełny i 2× to szybkie wybory. 100 % zachowuje
wszystkie szczegóły; pliki RAW wczytane z **RAW w połowie rozmiaru** są przy
zapisie odczytywane ponownie w pełnym rozmiarze, co wymaga więcej czasu i
pamięci. 50 % lub 25 % szybko dają mniejszy plik; powyżej 100 % powiększa (bez
nowych szczegółów).

Pod rozmiarem szacunek pokazuje, ile pamięci potrzebuje zapis i jak duży może
być plik, obok tego, co jest wolne. Zmienia się na pomarańczowo (⚠), gdy się
nie mieści; najedź na niego, by zobaczyć, co zrobić. Zapis wtedy odmawia z tą
samą radą, zamiast zapełnić pamięć, przez co cały komputer przestałby
odpowiadać. Wczytywanie zatrzymuje się wcześniej, z komunikatem, gdy pamięć
jest prawie pełna.

Duża panorama jest renderowana pasami wierszy, gdy nie mieści się w pamięci w
całości; pasek stanu liczy wtedy pasy. W całości trzymana jest tylko gotowa
panorama, w formacie pliku (3 bajty na piksel dla JPEG, 6 dla 16-bitowego TIFF
i PNG), a zdjęcia odczytane ponownie w pełnym rozmiarze są trzymane w połowie
precyzji. Dlatego 16-bitowy TIFF 20 000 × 8000 px potrzebuje około 1 GB plus
jeden pas i zdjęcia, gdzie wcześniej potrzebował około 8 GB. Przy mieszaniu
**inteligentne** każdy pas jest renderowany z zakładką, więc pasy łączą się bez
widocznego stopnia. Zapis nigdy nie używa więcej niż 85 % pamięci.

Panoramy szersze lub wyższe niż 60 000 px są odrzucane. To prawie zawsze
oznacza, że jedna para jest źle dopasowana albo projekcja jest nieodpowiednia;
spróbuj cylindrycznej lub sferycznej.

Gdy łączenie trwa, pasek stanu pokazuje upływający czas. Po zapisie mały
**raport** pod przyciskiem zapisu podsumowuje: ile zdjęć jest w panoramie (i
ile się nie dopasowało), czas łączenia (wczytywanie, dopasowanie i podglądy od
dodania pierwszego zdjęcia), czas zapisu, rozmiar w pikselach i plik z jego
rozmiarem albo błąd, jeśli zapis się nie udał. × go zamyka.

---

## 11. Rozwiązywanie problemów

| Problem | Co spróbować |
|---|---|
| **Okno się nie pojawia** | W Windows komunikat mówi dlaczego; w Linuksie i macOS uruchom program z terminala i przeczytaj tam błąd. Okno wymaga OpenGL 2.0 lub nowszego: zaktualizuj sterownik grafiki (w starych komputerach ten od producenta układu graficznego). |
| **Program nieoczekiwanie się zamknął** | Przy następnym uruchomieniu mówi o tym pasek stanu. Szczegóły są w `crash.log` w folderze ustawień (Linux `~/.config/panostitch`, Windows `%APPDATA%\panostitch`, macOS `~/Library/Application Support/panostitch`); dołącz go, zgłaszając problem. |
| **Elementy ucięte z boku** | Przeciągnij krawędź między panelem bocznym a obrazem, by go poszerzyć. |
| **„Zajęty – poczekaj…”** | Krok wciąż trwa; poczekaj, aż zmieni się pasek stanu. |
| **Zdjęcie jest pomijane przy wczytywaniu** | Pasek stanu podaje plik i powód. Dla RAW spróbuj innego konwertera w **RAW przez**. Dla FITS `.fz` najpierw rozpakuj plik. |
| **Panorama jest prawie czarna** | Zdjęcia: kliknij **Automatyczna ekspozycja** lub zwiększ **Ekspozycja (EV)** (zdjęcia nocne często potrzebują +1,5 do +4) albo wyedytuj zdjęcia w RawTherapee (wtedy używane są ich pliki `.pp3`). FITS i inne dane liniowe: kliknij **Automatyczne rozciągnięcie**. |
| **Obrazy FITS po wczytaniu są czarne lub białe** | Automatyczne rozciągnięcie przy wczytaniu bazuje na wszystkich obrazach FITS; jedna bardzo prześwietlona lub niedoświetlona klatka może je rozregulować. Usuń takie klatki, potem kliknij **Automatyczne rozciągnięcie** (lub **Resetuj rozciąganie** i ustaw ręcznie). |
| **Obrazy FITS są zielone** | Surowe klatki z kamer kolorowych nie mają balansu bieli. Najpierw je zestackuj i skalibruj kolory albo popraw kolor wyniku w programie astronomicznym. |
| **Mało żółtych punktów** | Obniż **Próg narożników**; zwiększ **Maks. punktów**; zwiększ **Rozmiar wykrywania (px)**. |
| **Punkty głównie na trawie, niebie lub szumie** | Zwiększ **Odszumianie (σ px)** do 2–3. |
| **Zdjęcia oznaczone UNLINKED** | Obejrzyj je: czy naprawdę zachodzą na inne zdjęcie? Zwiększ **Test proporcji** (0,85–0,9) i **Maks. odległość** (0,4), obniż **Min. inlierów**, zwiększ **Odszumianie (σ px)**, potem znów **Dopasuj**. Jeśli dalej się nie udaje, włącz **pokaż niepołączone zdjęcia** i rozmieść je przez **Ctrl+przeciąganie**. |
| **Dziura w zenicie lub klatki z samym niebem niepołączone (UNLINKED)** | Użyj plików RAW (potrzebne są ich ogniskowa i czas wykonania), zostaw **wszystkie pary** i znów kliknij **Dopasuj**: klatki z samym niebem są wtedy dopasowywane po gwiazdach. Każda klatka nieba potrzebuje kilkudziesięciu gwiazd i dobrego zachodzenia na sąsiednie. |
| **Rzędy skierowane w dół (lub nadir) niepołączone** | Ziemia fotografowana z bliska przesuwa się przy najmniejszym ruchu aparatu (paralaksa), a trawa nocą to głównie szum. Fotografuj rzędy z ziemią z głowicy panoramicznej obracającej się wokół obiektywu albo zasłoń nadir później. |
| **Chmury, woda lub inne ruchome rzeczy** | Zmieniają się między ujęciami, więc nie da się ich pewnie dopasować (klatki z samymi chmurami zwykle zostają niepołączone). Fotografuj takie części szybko jedna po drugiej albo rozmieść je ręcznie. |
| **Używana jest zła para** (linie we wszystkich kierunkach, zdjęcia w złym miejscu) | Wybierz ją w menu **Para** i kliknij **Odrzuć parę**. Z włączonym **dopracuj** większość takich par jest odrzucana automatycznie. |
| **Duchy (podwojone obiekty)** | Zostaw włączone **dopracuj** i użyj **Mieszanie → inteligentne** (domyślnie). Pozostałe problemy wynikają z rzeczy, które zmieniły się między ujęciami (np. latarka włączona tylko na jednym zdjęciu), albo z **paralaksy**: bliskie obiekty przesuwają się względem tła, gdy aparat nie obraca się wokół obiektywu („punkt nodalny”). |
| **Widoczne skoki jasności** | **Wyrównaj**, potem dostrój każde zdjęcie przez **Jasność (zazn.)**. |
| **Wygięty horyzont** | Ustaw **Pochylenie (°)** (i **Przechył (°)**, jeśli jest przekrzywiony). |
| **Mocno rozciągnięta góra lub dół** | Użyj projekcji **sferyczna**; pasek stanu ją podpowiada, gdy panorama sięga daleko w górę lub w dół. |
| **Panorama ściśnięta lub rozciągnięta** | Ustaw **Ogniskowa (× auto)**. |
| **Ogromna, zniekształcona panorama** | Zła para (sprawdź pary) albo użyj **sferyczna** / **cylindryczna** zamiast prostoliniowej. |
| **Wolno lub brak pamięci** | Włącz **RAW w połowie rozmiaru** przed wczytaniem; zmniejsz **Rozmiar podglądu (px)**. Zdjęcia w pełnej rozdzielczości potrzebują około 12 bajtów na piksel każde, około 300 MB dla zdjęcia 24 MP. Przy wielu zdjęciach dopasowanie *wszystkie pary* trwa dłużej; *siatka* jest szybsza, gdy znasz układ. |
| **Automatyczne kadrowanie zostawia tylko wąski pasek** | Panorama jest wygięta. Najpierw ją wyprostuj przez **Pochylenie (°)** i **Przechył (°)** (i wybierz odpowiednią projekcję), potem znów **Automatyczne kadrowanie**. |
| **Projekt otwiera się bez niektórych zdjęć** | Pasek stanu wymienia brakujące pliki. Zostały przeniesione lub przemianowane, i to nie razem z plikiem projektu. Przywróć je albo dodaj je ponownie i dopasuj. |
| **Cofanie nic nie robi** | Cofanie działa na ustalonych zmianach; poczekaj chwilę po ostatniej zmianie. Gdy krok trwa (*Zajęty*), cofanie czeka. |
| **Wszystko jest wolne** | Włącz **RAW w połowie rozmiaru** przed wczytaniem (RAW i FITS z kamer kolorowych); zmniejsz **Rozmiar podglądu (px)**. W karcie stackowania **Połowa rozmiaru** w Konwersji RAW. |

---

## 12. Wskazówki dla dobrych panoram

- **Nakładaj** sąsiednie ujęcia o 30–50 %, także między rzędami. Dla mozaik z
  teleskopu zwykle wystarcza 15–25 %.
- **Obracaj aparat** wokół obiektywu, nie wokół siebie. Używaj statywu, jeśli
  możesz, najlepiej z głowicą panoramiczną.
- **Zablokuj ekspozycję, balans bieli i ostrość** (tryb ręczny), żeby
  wszystkie klatki pasowały.
- **Zaklej pierścień zoomu** przy nocnych panoramach: skierowany w górę zoom
  może się przesunąć. PanoStitch sobie z tym radzi, ale stała ogniskowa
  dopasowuje się najlepiej.
- Unikaj ruchomych obiektów w obszarach zakładki albo uchwyć je tylko na
  jednej klatce.
- **Trzymaj aparat w poziomie.** Przechylony aparat wygina horyzont (poprawia
  się to pochyleniem i przechyłem).
- Przy długich seriach najpierw sprawdź wynik z włączonym **RAW w połowie
  rozmiaru**.
- Nie zapisuj wyniku w folderze zdjęć źródłowych, jeśli wczytujesz zdjęcia
  folderem. Pliki o nazwie `panorama…` są pomijane, ale inne nazwy nie.

---

## 13. Stackowanie gwiazd

Druga karta, **Stackowanie gwiazd (krajobraz / ślady)**, łączy wiele
ekspozycji tej samej sceny w jeden obraz: niebo o niskim szumie z ostrymi
gwiazdami albo ślady gwiazd. Jest zrobiona do astrofotografii krajobrazowej:
aparat stoi na statywie, niebo przesuwa się między klatkami, pierwszy plan
nie.

Działa jak Sequator, z tym, czego Sequatorowi brakuje: biasy i flat darki,
32-bitowy wynik liniowy, osobne zdjęcie pierwszego planu, ślady gwiazd bez
przerw, automatyczne wykrywanie nieba i żadnej czarnej skrzynki: każda klatka
jest mierzona i oceniana, wykres pokazuje pomiary, a dziennik wymienia każdy
krok z tym, co zrobił i ile trwał.

```
┌──────────────────────┬──────────────────────────────────────┬─────────────────────┐
│ input list           │ [Base image] [Result] [Frame report] │ Post-processing     │
│  Star images, Base,  │ [Fit] [Open project…] [Save project…]│  Background extr.   │
│  calibration, Fore-  │                                      │  Color calibration  │
│  ground, Output      │                                      │                     │
│ Add Remove Clear     │   base image (draw the sky here),    │  Colour noise       │
│ Use as base          │   result, or frame report            │  Light pollution    │
│ settings list        │                                      │  Brightness, Tone   │
│ options of the       │                                      │  Color space        │
│ highlighted entry    │                                      │  [Save result…]     │
│                      │                                      │  [Use in Stitcher]  │
│ [Quick prev.] [Start]│  status line                         │                     │
└──────────────────────┴──────────────────────────────────────┴─────────────────────┘
```

**Kliknij wpis** na liście wejść lub liście ustawień: pod listami pojawią się
jego opcje i objaśnienie.

**Ustawienia** (nad listą ustawień) ustawiają wszystko dla typowego tematu
jednym kliknięciem: **Krajobraz z Drogą Mleczną** (zamrożona ziemia, niebo
wykrywane automatycznie, korekcja dystorsji, zanieczyszczenie światłem),
**Ślady gwiazd** (wypełnione przerwy) i **Deep sky (teleskop)** (całe niebo,
ekstrakcja tła, kalibracja kolorów, automatyczne rozciąganie). Każde
ustawienie można dalej zmieniać. W trakcie przebiegu pasek postępu pod
widokiem pokazuje krok, upływający czas i szacowany pozostały czas. Po
zakończeniu u góry prawego panelu pojawia się mały **raport**: ile klatek
zestackowano z ilu (z pominiętymi i wykluczonymi), klatki kalibracyjne, ile to
trwało, rozmiar wyniku i zapisany plik z jego rozmiarem albo co poszło źle.
× go zamyka.

### 13.1 Szybki start

1. Zaznacz **Lighty**, kliknij **Dodaj…** i wybierz zdjęcia gwiazd (albo wpisz
   ścieżkę pliku lub folderu w pole ścieżki i naciśnij Enter).
2. Zaznacz **Obszar nieba**, wybierz **Linia graniczna** i przeciągnij linię
   wzdłuż horyzontu w widoku klatki bazowej albo kliknij **◎ Wykryj niebo** i
   popraw maskę pędzlem. Gwiazdy są szukane tylko na niebie.
3. Kliknij **◐ Szybki podgląd**: mały, szybki wynik z najwyżej 10 klatek, do
   sprawdzenia ustawień. Nic nie jest zapisywane.
4. Sprawdź wiersz **Zapisz jako** pod listą wejść: wpisz nazwę pliku lub wybierz
   jedną z proponowanych nazw z listy obok, wybierz typ pliku z drugiej listy,
   a folder przyciskiem **Folder…** (domyślnie: `stacked.tif` obok pierwszego
   zdjęcia gwiazd). Jeśli plik już istnieje, wiersz o tym mówi; **następna
   wolna nazwa** dodaje numer. Pod nim szacunek pokazuje, ile miejsca klatki
   potrzebują podczas stackowania (z ich liczby i rozmiaru) i czy są trzymane w
   pamięci, czy na dysku; zmienia się na pomarańczowo (⚠), gdy miejsca jest za
   mało. Potem kliknij **▶ Start**.
5. Ustaw obróbkę po prawej; wynik aktualizuje się od razu. Kliknij
   **⬇ Zapisz wynik…**, by zapisać go ponownie z nowymi ustawieniami.

### 13.2 Pliki wejściowe

| Wpis | Co sfotografować |
|---|---|
| **Lighty** (*Zdjęcia gwiazd* w trybie prostym) | Ujęcia nieba: ta sama ogniskowa, przysłona i ekspozycja. Najlepszy jest RAW; TIFF, FITS lub XISF tylko, jeśli dane są wciąż liniowe (nieedytowane). **Ujednolić ekspozycję** wyrównuje klatki zrobione w innych warunkach. |
| **Klatka bazowa** (*Zdjęcie bazowe* w trybie prostym) | Klatka, do której wyrównywane są pozostałe. Puste = środkowy light. Wybierz inną z listy obok **Klatka bazowa** albo wybierz light z jego listy i kliknij **Użyj jako bazowej**. |
| **Darki** (*Zdjęcia szumu* w trybie prostym) | Ciemne klatki: dekielek na obiektywie, to samo ISO i czas, zrobione tuż przed zdjęciami gwiazd lub po nich. Usuwają szum termiczny i gorące piksele. |
| **Biasy** (*Zdjęcia bias* w trybie prostym) | Najkrótszy czas, dekielek na obiektywie, to samo ISO. |
| **Flaty** (*Zdjęcia winietowania* w trybie prostym) | Klatki flat: równomiernie oświetlona biała powierzchnia, ta sama ogniskowa i przysłona. Korygują ciemniejsze rogi i cienie kurzu. |
| **Flat darki** (*Zdjęcia flat dark* w trybie prostym) | Darki do flatów: to samo ISO i czas co flaty. |
| **Zdjęcie pierwszego planu** | Opcjonalne: jedno ujęcie ziemi z tego samego miejsca statywu – o niebieskiej godzinie, z długim czasem lub niskim ISO. Zastępuje (zaszumiony, ciemny) stackowany pierwszy plan pod obszarem nieba. **Dopasuj jasność do stackowanej ziemi** skaluje je do stackowanej ziemi; **Ekspozycja (EV)** rozjaśnia lub przyciemnia je od tego punktu. Wymaga obszaru nieba. |
| **Wynik** | Ustawiany w wierszu **Zapisz jako**: nazwa, typ i folder. `.tif` / `.png` (16 bitów), `.jpg` (8 bitów) lub `.fits` / `.xisf` (32-bitowe liczby zmiennoprzecinkowe, liniowo, do dalszej obróbki w Sirilu, PixInsight itp.). |

Wszystkie klatki kalibracyjne są opcjonalne. Nawet kilka darków bardzo pomaga
przy wysokim ISO.

**Listy plików:** każda grupa pokazuje swoje pliki na liście obok swojej nazwy,
więc setki lightów nie wymagają przewijania. Wybierz tam plik, by zobaczyć go
w podglądzie; **Usuń** i **Użyj jako bazowej** działają na tym pliku,
**Wyczyść** opróżnia całą grupę.

**Zaszumione flaty** (za mało ich albo za ciemne w jednym kolorze, np. przy
niebieskim źródle światła) są automatycznie wygładzane; dziennik o tym mówi i
podaje, jak bardzo były zaszumione. Inaczej ich szum odcisnąłby się na każdej
klatce i zamienił w drobne smugi, gdy gwiazdy dryfują.

**Pliki tymczasowe:** dla obrazów pośrednich nie są zapisywane żadne. Klatki są
trzymane w pamięci albo, jeśli zajmują więcej niż połowę wolnej pamięci, w
jednym pliku tymczasowym w folderze wyniku, usuwanym zaraz po zakończeniu
przebiegu (nie widać go nawet w trakcie). Dziennik mówi, która opcja została
użyta. Jeśli miejsca nie wystarczy, przebieg zatrzymuje się z komunikatem
przed startem: włącz **Połowa rozmiaru** (ćwierć miejsca) albo zapisz wynik na
dysku z większą ilością wolnego miejsca. Mastery (dark, bias, flat) też nie są
zapisywane.

### 13.3 Ustawienia
| Ustawienie | Co robi |
|---|---|
| **Kompozycja** | **Wyrównaj gwiazdy**: klatki są wyrównywane do gwiazd i łączone. **Akumulacja** je uśrednia; **Wybierz najlepsze piksele** (domyślnie) odrzuca wartości odstające (samoloty, satelity); **Zamroź ziemię** wyrównuje tylko niebo, więc niebo i pierwszy plan zostają ostre (wymaga obszaru nieba); **Tylko wyrównaj** tylko zapisuje wyrównane klatki. **Ślady gwiazd**: bez wyrównania, niebo zachowuje najjaśniejszą wartość każdego piksela. **Zanikanie śladów**: **Pojawianie** (ślady zaczynają się słabo, efekt ruchu z Sequatora), **Zanikanie** lub **Oba końce** (zwężenie jak u komety). **Wypełnij przerwy między klatkami**: krótkie przerwy interwału aparatu są wypełniane wzdłuż zmierzonego ruchu gwiazd, więc ślady są ciągłymi liniami zamiast kresek. |
| **Obszar nieba** | Gdzie jest niebo. **Całe niebo**: cała klatka to niebo (bez pierwszego planu, np. ujęcia przez teleskop lub deep sky). **Linia graniczna**: przeciągnij linię wzdłuż horyzontu. **Gradient**: linia plus łagodne przejście. **Nieregularna maska**: maluj niebo lewym przyciskiem myszy, wymazuj prawym; **Rozmiar pędzla** (suwak lub kółko myszy). **◎ Wykryj niebo** maluje pierwszą maskę na podstawie jasności i gwiazd – w przybliżeniu, więc ją sprawdź i popraw pędzlem. **Rozmycie maski** zmiękcza krawędź maski, tylko wewnątrz nieba, więc pierwszy plan nic z nieba nie dostaje (bez poświat wzdłuż horyzontu). **Pokaż wykryte gwiazdy** zakreśla gwiazdy, które ostatni przebieg znalazł w pokazanej klatce. |
| **Sigma clipping** | Jak surowo usuwane są wartości odstające (dolny / górny × σ). Zwykle 2–3; niżej usuwa więcej, ale zostawia więcej szumu. Wymaga co najmniej 4 klatek, najlepiej 10+. |
| **Odrzucanie klatek** | Pomija klatki wyraźnie gorsze od typowej: chmury lub rosa (słabsze gwiazdy), mało gwiazd, rozmyte lub wydłużone gwiazdy (ostrość, wiatr, potrącony statyw). |
| **Konwersja RAW** | Balans bieli (z aparatu, średnia obrazu, światło dzienne), demozaikowanie (domyślnie AHD; DHT lub AAHD często dają mniej szumu kolorowego przy wysokim ISO) i **Połowa rozmiaru** (4× mniej pikseli, dużo szybciej). |
| **Redukuj szum dynamiczny** | Znajduje i usuwa gorące piksele bez darków. |
| **Redukuj efekty dystorsji** | Koryguje dystorsję obiektywu, która zostawia krótkie ślady w rogach długich serii: **Obiektyw**, **Złożona** (ultraszerokie kąty, refrakcja przy horyzoncie) lub **Auto**. |
| **Połącz 4 piksele** | Sumuje piksele 2×2: jaśniej i mniej szumu, połowa szerokości i wysokości. |
| **Timelapse** | Stackuje każde N kolejnych klatek w jedną, dając numerowaną sekwencję klatek o niskim szumie w folderze obok wyniku. |

### 13.4 Widoki

- **Klatka bazowa**: klatka, do której wyrównywane są pozostałe; tu rysuje się
  obszar nieba. Wybierz plik z listy grupy, by go zobaczyć.
- **Wynik**: stack po obróbce. Przeciągaj, by przesuwać, kółko myszy do
  powiększania, **Dopasuj**, by zobaczyć całość.
- **Raport klatek**: wykres każdego pomiaru dla każdej klatki, podzielonego
  przez jego medianę, by zła klatka się wyróżniała (czerwone = pominięte,
  niebieskie = klatka bazowa); pod nim jeden wiersz na każde zdjęcie gwiazd:
  znalezione gwiazdy, rozmiar gwiazd (FWHM), wydłużenie, poziom nieba,
  światło (jasność gwiazd względem klatki bazowej), błąd wyrównania, czy
  klatka została użyta i dlaczego nie. Pod tabelą **dziennik
  przetwarzania**: każdy krok przebiegu, co zrobił (kalibracja, obszar nieba,
  użyte klatki, dystorsja, wypełnianie przerw, pierwszy plan, obróbka) i ile
  trwał. **Kliknij klatkę** na wykresie lub w tabeli, by ją pominąć (lub znów
  uwzględnić), potem znów kliknij **▶ Start**.

### 13.5 Obróbka

Stosowana od razu do stackowanego wyniku; **▶ Start** zapisuje z tymi
ustawieniami, **⬇ Zapisz wynik…** zapisuje ponownie.

| Ustawienie | Co robi |
|---|---|
| **Ekstrakcja tła** | Odejmuje gładki model tła nieba (gradienty od zanieczyszczenia światłem lub księżyca). |
| **Kalibracja kolorów** | Czyni tło nieba neutralnie szarym, a przeciętną gwiazdę białą. |
| **Redukuj szum kolorowy** | Wygładza tylko kolory, usuwając kolorowe plamki ze zdjęć z wysokim ISO. |
| **Zanieczyszczenie światłem** | **Deep sky**: usuwa gładką łunę. **Nierównomierne**: śledzi łunę kafelek po kafelku (łuny miast na ujęciach szerokokątnych). **Siła** ustala, ile jest usuwane. Najpierw skoryguj winietowanie (flaty), inaczej będzie gorzej. |
| **Wzmocnij światło gwiazd** | Wyciąga gwiazdy ponad tło nieba, nie wzmacniając jego szumu. |
| **Automatyczna jasność** | Ustawia niebo na przyjemnym poziomie bez przepalania najjaśniejszych gwiazd. |
| **Wysoki zakres dynamiki** | Kompresuje jasne gwiazdy i światła zamiast je obcinać. |
| **Ton** | **krzywa sRGB** (jak w aparacie), **Rozciąganie** (punkt czerni, półtony, punkt bieli; automatycznie lub ręcznie) lub **Liniowy** (bez krzywej). |
| **Przestrzeń barw** | sRGB lub Adobe RGB dla wyniku TIFF i JPEG. |
| **Liniowy (do obróbki)** | Wyłącza wszystko i zapisuje liniowy 32-bitowy FITS: czego oczekują Siril lub PixInsight. |
| **⇄ Użyj w Łączeniu panoram (fragment panoramy)** | Dodaje zapisany wynik do karty Łączenie panoram jako jeden fragment panoramy (zob. 13.7). |

### 13.6 Projekty

**⬇ Zapisz projekt…** zapisuje plik `.stackproj` z listami plików, obszarem
nieba i każdym ustawieniem. **⇪ Otwórz projekt…** (lub
`panostitch noc.stackproj`) przywraca go. Pliki są odnajdywane po pełnej
ścieżce albo względnie do pliku projektu, jeśli folder został przeniesiony.

### 13.7 Panoramy ze stackowanych fragmentów

Panorama Drogi Mlecznej o niskim szumie: zrób kilka klatek w każdej pozycji
panoramy (z zakładką między pozycjami), potem:

1. Zestackuj klatki z pierwszej pozycji (Start), potem kliknij **⇄ Użyj w
   Łączeniu panoram (fragment panoramy)**.
2. Zastąp zdjęcia gwiazd tymi z następnej pozycji, nadaj wynikowi nową nazwę,
   Start, znów **⇄ Użyj w Łączeniu panoram (fragment panoramy)**; i tak dalej.
3. W karcie Łączenie panoram **▶ Wykonaj wszystkie kroki** łączy stackowane
   fragmenty.

Zapisz każdy fragment jako `.tif`; ta sama obróbka dla wszystkich fragmentów
utrzymuje jednakową jasność i kolory.

### 13.8 Wskazówki

- Zrób co najmniej 10–20 zdjęć gwiazd z tymi samymi ustawieniami, plus 10–20
  darków zaraz potem, bez ruszania aparatu.
- Ustaw obszar nieba starannie: drzewa i budynki liczone jako niebo rozmywają
  się przez wyrównanie. **◎ Wykryj niebo** to szybki start, nie ostatnie
  słowo.
- Po szybkim podglądzie zajrzyj do Raportu klatek: wykres od razu pokazuje,
  które klatki są gorsze (chmury, wiatr, rosa).
- Używaj **Zamroź ziemię**, gdy pierwszy plan ma znaczenie, **Wybierz
  najlepsze piksele**, gdy nie.
- Do obróbki w innych programach zapisz jako `.fits` i włącz **Liniowy (do
  obróbki)**.
- Z terminala `panostack` stackuje bez okna; `panostack --help` wymienia jego
  opcje (zob. [Użycie bez okna](#14-użycie-bez-okna)).

---

## 14. Użycie bez okna

**Stackowanie gwiazd z terminala:** `panostack` wykonuje to samo stackowanie
co karta Stackowanie gwiazd. `panostack --help` wymienia wszystkie opcje.
Przykłady:

```
panostack --mode freeze --horizon 0.62 --darks darks/ -o noc.tif lights/
panostack --mode trails --fade in --fill-gaps --auto-sky -o slady.jpg lights/
panostack --preview lights/            # szybkie sprawdzenie, nic nie jest zapisywane
```

Raport klatek i dziennik przetwarzania są wypisywane na końcu.

**Łączenie z własnego programu w Rust:** dodaj PanoStitch jako zależność
(`panostitch = { path = "…/PanoStitch" }` w swoim `Cargo.toml`), potem:

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

`examples/scripted.rs` to kompletny przykład
(`cargo run --release --example scripted -- a.NEF b.NEF c.NEF`).
`stitch_files(paths, params, raw_half, log)` wczytuje i łączy;
`stitch(images, params, log)` łączy zdjęcia już będące w pamięci.
Pola `StitchParams` z wartościami domyślnymi:

| Pole | Domyślnie | Znaczenie |
|---|---|---|
| `layout` | Layout::All | które pary dopasowywać: `All`, `Sequential`, `Grid` |
| `cols`, `snake` | 0 (wszystkie zdjęcia), `false` | układ siatki |
| `refine` | `true` | globalne dopracowanie |
| `feature_scale` | `0.5` | skala wykrywania |
| `nkeypoints` | `5000` |  |
| `denoise` | `1.5` |  |
| `ratio` | `0.8` |  |
| `max_distance` | `0.35` |  |
| `model` | `Model::Homography` | `Homography`, `Affine`, `Similarity`, `Translation` |
| `ransac_thresh` | `5.0` |  |
| `min_inliers` | `20` |  |
| `projection` | `Projection::Cylindrical` | dowolna z [Projekcji](#7-projekcje), np. `Fisheye`, `LittlePlanet`, `Mollweide` |
| `blend` | `Blend::Smart` | `Smart`, `Feather`, `Seam`, `Average` |
| `sharpness` | `3.0` | tryb przenikania: wyżej = węższe przejścia |

**Mozaika FITS z rozciągniętym TIFF i liniowym wynikiem FITS:**

```rust
use panostitch::stitcher::{Model, Projection, StitchParams};

let p = StitchParams { model: Model::Similarity, refine: false,
                       projection: Projection::Rectilinear, denoise: 0.75, ..Default::default() };
let mosaic = stitcher::stitch_files(&tiles, &p, false, &mut |_| {})?;
io::save_panorama("mosaic.fits", &mosaic)?;                          // liniowo
let (black, mid, white) = stitcher::auto_stretch(&mosaic, 0.25, 2.8);
io::save_panorama("mosaic.tif", &stitcher::hist_stretch(&mosaic, black, mid, white))?;
```

Stackowanie gwiazd z kodu: wypełnij `stacker::StackConfig` (zaczynając od
`StackConfig::default()`) i wywołaj
`stacker::stack(&config, false, &progress, None)`.

---

## 15. Pliki

| Plik | Do czego służy |
|---|---|
| `panostitch` (`panostitch.exe`) | Program (okno). |
| `panostack` (`panostack.exe`) | Stackowanie gwiazd z wiersza poleceń (`panostack --help`). |
| `*.panoproj`, `*.stackproj`, `*.tlproj` | Twoje pliki projektów (gdziekolwiek je zapiszesz). |
| `ui.toml` w folderze ustawień | Przełączniki wyglądu i język (zob. [Uruchamianie programu](#2-uruchamianie-programu)). |
| `remembered/`, `recent_projects.txt`, `last_folder.txt` w folderze ustawień | Ustawienia zapamiętane na następną sesję, ostatnie projekty, folder, w którym zaczynają okna wyboru plików. Usuń je, by zacząć od nowa. |
| `docs/manual_pl.md` | Ten podręcznik (polski). |
| `docs/manual_en.md`, `docs/manual_cs.md`, `docs/manual_de.md`, `docs/manual_es.md`, `docs/manual_fr.md` | Podręcznik po angielsku, czesku, niemiecku, hiszpańsku i francusku. |

Kod źródłowy (by samemu zbudować program):

| Ścieżka | Zawartość |
|---|---|
| `Cargo.toml` | Projekt i biblioteki, których używa. |
| `src/image.rs`, `src/io/` | Obrazy; wczytywanie i zapis (JPEG, PNG, TIFF, BMP, RAW, FITS, XISF). |
| `src/stitcher/` | Łączenie: punkty, dopasowanie, rozmieszczenie, dopracowanie, projekcje, mieszanie. |
| `src/stacker/` | Stackowanie gwiazd. |
| `src/finish.rs` | Obróbka: geometria, rozciągania (MTF, arcsinh, GHS), balans bieli, światło, krzywe, kolor, szczegóły. |
| `src/gui/` | Okno: cztery karty, okna wyboru plików, wbudowany podręcznik i teksty okna w sześciu językach (`i18n_*.rs`). |
| `src/bin/` | Dwa programy, `panostitch` i `panostack`. |
| `tests/` | Testy: stackowanie na syntetycznych scenach nocnych, łączenie, obróbka, formaty plików i karty okna. Uruchom je przez `cargo test --release`. |
| `examples/scripted.rs` | Używanie łączenia z własnego kodu w Rust. |

---

## 16. Obróbka

*Nowość w 0.4 (w rozwoju).* Karta **Obróbka** wykonuje ostatnie kroki na
stackowanym lub połączonym obrazie, żeby nie był potrzebny inny program: obrót
i kadrowanie, rozciąganie danych liniowych, balans bieli, światło, krzywe,
kolor, szum, gwiazdy, wyostrzanie i zapis w wybranym rozmiarze.
**Wczytanie obrazu:** **Otwórz…** (Ctrl+O) lub wpisz ścieżkę i naciśnij
Enter; **✎ Użyj w Obróbce** w karcie Stackowanie gwiazd; albo **✎ Obróbka** w
łączeniu panoram po zapisaniu panoramy. Pliki FITS i XISF są traktowane jako
dane liniowe i zaczynają z automatycznym rozciągnięciem; JPEG, TIFF i PNG są
traktowane jako gotowe obrazy (bez rozciągania). **Dane liniowe (wymagają
rozciągnięcia)** przełącza to ręcznie.

Podgląd działa na kopii o najwyżej 2000 px, więc nadąża za suwakami niemal od
razu. **⬇ Zapisz** przetwarza cały obraz w tle z dokładnie tymi samymi
ustawieniami; wartości mierzone przez program (automatyczne rozciągnięcie,
neutralne tło) są brane z podglądu, więc zapisany plik wygląda jak podgląd.

Kroki zawsze wykonywane są w tej kolejności, od góry do dołu po lewej:

| Sekcja | Co robi |
|---|---|
| **Obrót i kadrowanie** | ⟲ ⟳ obroty o ćwierć, **Odbicie lustrzane**, **Obrót (°)** o dowolny kąt (dokładnie przy 0°, jak odchylenie/pochylenie/przechył w łączeniu), **Odetnij puste narożniki** po obrocie, przycinanie z każdej strony lub do kształtu (3:2, 4:3, 16:9, 4:5, 1:1, 2:1). |
| **Rozciąganie** (dane liniowe) | Najpierw **Ekstrakcja tła** i **Kalibracja kolorów** (obie opcjonalne, na danych liniowych), potem metoda rozciągania (niżej). |
| **Balans bieli** | Na liniowym świetle, przed rozciąganiem. **◎ Próbka szarości**, potem przeciągnij prostokąt na czymś, co powinno być neutralne (tło nieba, chmura): z tego ustawiane są mnożniki kolorów. **Neutralne tło** czyni ciemniejszą połowę szarą. **Temperatura** i **Odcień** dostrajają. Gotowe obrazy są do tego wyjmowane z krzywej sRGB i potem do niej wracają, więc balans jest właściwy przy każdej jasności. |
| **Światło** | **Ekspozycja (EV)**, **Kontrast**, **Światła** (około 75 %), **Półtony** (50 %), **Cienie** (25 %), **Biele** i **Czernie** (punkty końcowe), jak w edytorach zdjęć. |
| **Krzywe** | Jedna krzywa dla wszystkich kolorów (**RGB**) i po jednej dla **Czerwony**, **Zielony**, **Niebieski**. Kliknij, by dodać punkt, przeciągnij, by go przesunąć, prawy przycisk, by usunąć. Histogram za krzywą dotyczy tego, co do niej wchodzi. • oznacza zmienione krzywe. |
| **Kolor** | **Nasycenie** (0 = czarno-białe) i **Jaskrawość** (matowe kolory bardziej niż mocne). |
| **Szczegóły** | **Redukcja szumu** (wygładza płaskie obszary, zachowuje krawędzie i gwiazdy), **Szum kolorowy**, **Redukcja gwiazd** (mniejsze gwiazdy), **Wyostrz** i jego promień. Promienie są w pikselach pełnego obrazu, więc podgląd odpowiada zapisanemu wynikowi. |
| **Niebo i pierwszy plan** | Osobny wygląd nieba i pierwszego planu, na dodatek do wszystkiego powyżej, przez maskę (zob. niżej). |

**Metody rozciągania:**

| Metoda | Zastosowanie |
|---|---|
| **Wył.** | Obraz jest już gotowy. |
| **krzywa sRGB** | Jak zrobiłby aparat: dla danych liniowych, które potrzebują tylko zwykłej gammy. |
| **MTF, automatycznie** | Punkt czerni tuż pod tłem nieba, półtony tak, by tło wypadło na jasność **Tło** (jak AutoStretch w Sirilu / STF w PixInsight). **Każdy kolor osobno** (domyślnie) mierzy czerwony, zielony i niebieski osobno, żeby zafarb stacku z aparatu nie zasłonił obrazu; same dane się nie zmieniają. **Auto → ręcznie** kontynuuje ręcznie od zmierzonych wartości. |
| **MTF, ręcznie** | Punkt czerni, półtony i punkt bieli (funkcja przenoszenia półtonów). |
| **Arcsinh** | Odwrotny sinus hiperboliczny: silne rozciąganie zachowujące kolory gwiazd. **Współczynnik rozciągania** 1–10000; głębokie stacki ze słabym tłem potrzebują kilku tysięcy. |
| **Uogólnione hiperboliczne (GHS)** | **Rozciąganie ln(D+1)** (siła), **Intensywność lokalna b** (jak wąsko rozciąganie skupia się wokół punktu symetrii), **Punkt symetrii** (gdzie rozciąganie jest najsilniejsze: ustaw go na tle nieba), **Chroń cienie** i **Chroń światła** (prosta poniżej / powyżej nich). |

Dla Arcsinh i GHS: **Odwrotna** używa krzywej odwrotnej (kompresuje zamiast
rozciągać, np. by cofnąć zbyt mocne rozciągnięcie); **Zachowaj kolory gwiazd**
rozciąga jasność i zachowuje kolor każdego piksela (wyłączone: każdy kolor
osobno, bardziej nasycone niebo, bledsze gwiazdy); **Punkty z tła** ustawia
punkt czerni tuż pod tłem najciemniejszego koloru, a dla GHS punkt symetrii na
tle.

**Wyświetlanie** (nad podglądem) zmienia tylko sposób pokazania podglądu, nigdy
zapisany obraz, jak tryby wyświetlania w Sirilu: **Liniowy (jak zapisany)**,
**Logarytm**, **Pierwiastek**, **Kwadrat**, **Asinh**, **AutoStretch**,
**Wyrównanie histogramu**. Przydatne, by zajrzeć w cienie lub sprawdzić tło
bez zmieniania czegokolwiek. „tylko widok” jest pokazane, dopóki nie jest
Liniowy.

**◐ Porównaj** (przytrzymaj go albo przytrzymaj B) pokazuje punkt wyjścia:
obrócony, przycięty i rozciągnięty, nic więcej.

**Zapis** (po prawej): **Rozmiar** (pełny lub dłuższy bok, np. 2048 px na
fora), nazwa i typ pliku, **Jakość JPEG**. Wiersz poniżej mówi, ile pamięci
wymaga zapis. FITS / XISF dostają gotowy (rozciągnięty) obraz jako 32-bitowe
liczby zmiennoprzecinkowe. **Zachowaj ustawienia dla następnego obrazu** (po
lewej) daje kilku obrazom ten sam wygląd, np. fragmentom panoramy.

| Klawisz | Działanie |
|---|---|
| Ctrl+O | Otwórz obraz |
| Ctrl+S | Zapisz (pod pokazaną nazwą pliku) |
| Ctrl+Shift+S | Zapisz jako… |
| F | Dopasuj podgląd |
| B (przytrzymaj) | Porównaj z punktem wyjścia |
| Ctrl+Z, Ctrl+Y | Cofnij, ponów |

**Niebo i pierwszy plan** (na początku zwinięte): krajobraz pod gwiazdami
często wymaga innego potraktowania obu części, np. mniej szumu i mniejszych
gwiazd na niebie, jaśniejszego i cieplejszego pierwszego planu. Namaluj maskę
przez **✎ Maluj** (lewy przycisk = niebo, prawy = pierwszy plan, kółko myszy =
rozmiar pędzla) albo pozwól **◎ Wykryj niebo** ją znaleźć (niebo u góry; popraw
ją pędzlem; dla okrągłego rybiego oka użyj **◯ Koło rybiego oka**, rozdział
17), z **Całe niebo**, **Odwróć** i **Wyczyść**, by zacząć od nowa. Potem wybierz
**Niebo** lub **Pierwszy plan** i ustaw jego światło, temperaturę, kolor i
szczegóły. **Miękka krawędź** łączy oba wyglądy na granicy; **Pokaż maskę**
barwi niebo na niebiesko, a pierwszy plan na pomarańczowo (zawsze widoczne
przy malowaniu). Maska leży na samym obrazie, więc zostaje na miejscu przy
obrocie lub kadrowaniu. Zapisany plik dostaje ten sam wygląd; • za nazwą
sekcji pokazuje, że jest w użyciu.

**↶ Cofnij** i **↷ Ponów** (Ctrl+Z, Ctrl+Y lub Ctrl+Shift+Z) przechodzą wstecz
i naprzód przez zmiany; przeciągnięcie suwaka lub pociągnięcie pędzlem to
jeden krok, a podpowiedź przycisku mówi, co cofa.

**✂ Kadruj** przycina na obrazie: pokazany jest cały obraz z ramką; przeciągnij,
by narysować nową ramkę, przeciągnij jej krawędzie lub narożniki, by ją
dopasować, przeciągnij wewnątrz, by ją przesunąć. Kliknij **✂ Kadrowanie:
gotowe**, by zobaczyć wynik. Przyciski kształtów i suwaki przycinania w
**Obrót i kadrowanie** ustawiają tę samą ramkę.

**Ustawienia** (na górze, pod obrazem): wpisz nazwę i **Zapisz**, by zachować
bieżący wygląd: rozciąganie, balans bieli, światło, krzywe, kolor, szczegóły i
wygląd nieba i pierwszego planu, ale nie obrót, kadrowanie ani rozmiar
eksportu (należą do jednego obrazu). **Ustawienie** stosuje zapisany wygląd do
otwartego obrazu (Ctrl+Z cofa); **Usuń** usuwa ustawienie o wpisanej nazwie.
Ustawienia są trzymane w folderze ustawień (`presets`).

## 17. Timelapse

*Nowość w 0.4 (w rozwoju).* Karta **Timelapse** tworzy film z sekwencji
klatek: krajobraz pod nocnym niebem fotografowany ze statywu, z niebem
obracającym się nad nieruchomym pierwszym planem. Powolny zoom lub ruch jak na
sliderze przez kadr może dodać ruch kamery.

**Projekty:** **⬇ Zapisz projekt…** zachowuje klatki, wygląd, maskę (i koło
rybiego oka) oraz każde ustawienie wideo w pliku `.tlproj`;
**⇪ Otwórz projekt…** (Ctrl+O) przywraca wszystko, także po przeniesieniu
folderu. Ctrl+S zapisuje od razu do otwartego projektu. „Zacznij od wyglądu z
karty Obróbka” jest zapisywane jako przełącznik: używa obrazu otwartego w tej
chwili w Obróbce.

**Klatki:** dodaj pliki lub folder; są odtwarzane w kolejności nazw plików
(numeracja aparatu). Kliknij jedną, by pokazać ją w środku; ◀ ▶ przechodzą po
nich. Przy klatkach RAW **RAW przez** wybiera, jak są czytane: **wbudowany
(szybki)** (domyślnie) jest szybki i traktuje każdą klatkę tak samo;
**rawtherapee** stosuje edycje zapisane w plikach `.pp3` RawTherapee obok
klatek, ale trwa kilka sekund na klatkę (około dziesięć razy dłużej); **auto**
używa RawTherapee tylko dla klatek, które mają plik `.pp3`. Karta mówi, ile
klatek go ma. RawTherapee zapisuje `.pp3` dla każdego zdjęcia otwartego w jego
przeglądarce plików, więc jego obecność nie znaczy, że zdjęcie było edytowane.

**Wygląd:** te same suwaki światła i koloru co w Obróbce (ekspozycja,
kontrast, światła, półtony, cienie, biele, czernie, temperatura, odcień,
nasycenie, jaskrawość), ustawiane osobno dla **Cała klatka**, **Niebo** i
**Pierwszy plan**. Niebo i pierwszy plan działają na dodatek do wyglądu całej
klatki, tylko w swojej części maski; • oznacza zmienione części. Wybrana
klatka pokazuje wygląd (**Pokaż wygląd**; wyłączone pokazuje klatkę bez zmian).
Deflicker i ruch kamery widać tylko w wideo.

**Maska:** jak w Obróbce (rozdział 16): **✎ Maluj**, **◎ Wykryj niebo**,
**Całe niebo**, **Odwróć**, **Wyczyść**, **Miękka krawędź**, **Pokaż maskę**.
Jedna maska służy wszystkim klatkom, bo na statywie pierwszy plan się nie
rusza.

Dla timelapse całego nieba z okrągłym rybim okiem skierowanym w górę
**◯ Koło rybiego oka** znajduje oświetlone koło obrazu i robi z nieba dysk w
jego środku; pierścień wokół (horyzont, drzewa, światła) to pierwszy plan.
**Koło nieba** ustala, jaka część promienia koła jest niebem, a duża **Miękka
krawędź** (do 25 %) zamienia granicę w gradient promienisty. Resztę popraw
pędzlem, np. teleskop lub osobę wchodzącą w niebo.

**Wideo:**

| Ustawienie | Co robi |
|---|---|
| **Deflicker** | Wyrównuje skoki jasności i koloru między klatkami (kroki ekspozycji, zmieniające się światło, balans bieli aparatu). **Lekki** porównuje każdą klatkę z sąsiednimi (**Wygładzanie**, w klatkach); **Mocny** używa trzy razy więcej. Powolne zmiany, jak zmierzch, zostają. **Także kolor** wyrównuje każdy kolor osobno; **Obszarami** robi to na zgrubnej siatce w kadrze, więc zmiany w dużej skali w jednej części (zanieczyszczenie światłem, przejeżdżające światło, świt z jednej strony) są wyrównywane tam, gdzie się dzieją. Oba działają ze zwykłymi obiektywami i rybimi okami. Pomiar czyta każdą klatkę jeszcze raz. |
| **Zacznij od wyglądu z karty Obróbka** | Każda klatka dostaje najpierw ustawienia obrazu otwartego w Obróbce (rozciąganie, balans bieli, światło, krzywe, kolor, szczegóły; bez obrotu i kadrowania), potem wygląd ustawiony tutaj. Wartości zmierzone przez automatyczne rozciąganie pochodzą z pierwszej klatki i służą wszystkim, więc wideo nie migocze. |
| **Ruch jak na sliderze** | Jakby aparat jechał na zmotoryzowanym sliderze: wideo pokazuje fragment klatki o tyle procent mniejszy (do 20 %) i przesuwa go przez kadr, **Z lewej → w prawo**, **Z prawej → w lewo**, **W górę** lub **W dół**. Ruch jest płynny, nie skokami o cały piksel. |
| **Powolny zoom** | Powoli przybliża przez całe wideo; można łączyć z ruchem jak na sliderze. |
| **Łagodny start i stop** | Ruch i zoom przyspieszają na początku i zwalniają na końcu; wyłączone: cały czas ta sama prędkość. Podgląd zaznacza, gdzie wideo się zaczyna (jasno) i kończy (blado). |

**Wynik:**

| Ustawienie | Co robi |
|---|---|
| **Format** | **MP4** (H.264) i **WebM** (VP9) są tworzone przez ffmpeg. Zainstaluj go (ffmpeg.org; Windows: `winget install ffmpeg`; Linux: menedżer pakietów; macOS: Homebrew) albo umieść obok programu; karta mówi, gdzie go znalazła. **Animowany GIF** i **Numerowane obrazy** (TIFF lub PNG 16 bitów, JPEG) nie potrzebują niczego więcej. GIF-y pól gwiazd są duże (około 1 MB na klatkę w 720p; MP4 jest 5–10 razy mniejszy), więc nadają się do krótkich klipów. |
| **Klatki na sekundę** | 12–60 klatek na sekundę. |
| **Rozdzielczość** | Jak klatki, 4K, 1440p, 1080p lub 720p; nigdy nie powiększa. Do 1080p klatki RAW są czytane w połowie rozmiaru, co jest szybsze i wciąż wystarczająco ostre. |
| **Kształt** | Kształt wideo; klatka jest do niego przycinana, a podgląd zaznacza kadr. **Jak klatki** (bez przycinania), **16:9**, **3:2** i **4:3** (idą za orientacją klatek), **1:1 kwadrat** (całe koło rybiego oka całego nieba) lub **9:16 pionowo** (telefony). W 1080p: 1920 × 1080, 1620 × 1080, 1440 × 1080, 1080 × 1080 i 1080 × 1920. Z kołem rybiego oka karta ostrzega, gdy kształt odcina część nieba. |
| **Jakość** | Dla MP4, WebM i obrazów JPEG: wyżej = lepszy obraz, większy plik. |
| **Odtwórz do przodu, potem wstecz** | Na końcu klatki jeszcze raz w odwrotnej kolejności, więc wideo płynnie się zapętla. |
| **Zatrzymaj ostatnią klatkę** | Zostawia ostatnią klatkę na kilka sekund. |

Pod ustawieniami: ile klatek, jak długo trwa wideo i jego rozmiar w
pikselach. **▶ Utwórz wideo** pracuje w tle, do ośmiu klatek naraz (mniej, gdy
brakuje pamięci), z paskiem postępu (upływający i pozostały czas) i
**✖ Anuluj** (nic nie jest zachowywane). Potem raport pokazuje klatki, rozmiar
wideo, liczbę klatek na sekundę i długość, czas pracy oraz plik z jego
rozmiarem albo co poszło źle. Na przykład 69 klatek RAW (24 MP) dało film MP4
1080p z deflickerem w około pół minuty na komputerze stacjonarnym.
