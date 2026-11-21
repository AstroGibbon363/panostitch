# Panorama Stitcher: manual de usuario

*Otros idiomas: [English](manual_en.md) · [Česky](manual_cs.md) · [Deutsch](manual_de.md) · [Français](manual_fr.md) · [Polski](manual_pl.md)*

Un programa de escritorio que une fotos solapadas en una panorámica o un
mosaico: imágenes JPEG, TIFF, PNG, RAW de cámara y FITS astronómicas. Cada paso
se ejecuta solo cuando pulsa su botón, y puede revisar y corregir cada
resultado antes de seguir. Otras pestañas apilan tomas de estrellas en un
cielo nocturno con poco ruido o en trazas estelares ([Apilado de estrellas](#13-apilado-de-estrellas)),
dan el acabado a las imágenes ([Posprocesado](#16-posprocesado)) y hacen vídeos
([Timelapse](#17-timelapse)).

Los controles aparecen en este manual **en negrita**, tal como los ve en la
pantalla en español. El idioma de la ventana se cambia arriba a la derecha;
este manual también está dentro del programa, en **? Ayuda** (F1).

---

## Índice

1. [Requisitos](#1-requisitos)
2. [Iniciar el programa](#2-iniciar-el-programa)
3. [La ventana de un vistazo](#3-la-ventana-de-un-vistazo)
4. [Inicio rápido](#4-inicio-rápido)
5. [Paso a paso](#5-paso-a-paso)
6. [Moverse por la vista](#6-moverse-por-la-vista)
7. [Proyecciones](#7-proyecciones)
8. [Archivos RAW](#8-archivos-raw)
9. [Archivos FITS (astronomía)](#9-archivos-fits-astronomía)
10. [Guardar](#10-guardar)
11. [Solución de problemas](#11-solución-de-problemas)
12. [Consejos para buenas panorámicas](#12-consejos-para-buenas-panorámicas)
13. [Apilado de estrellas](#13-apilado-de-estrellas)
14. [Uso sin la ventana](#14-uso-sin-la-ventana)
15. [Archivos](#15-archivos)
16. [Posprocesado](#16-posprocesado)
17. [Timelapse](#17-timelapse)

---

## 1. Requisitos

- **No hay nada que instalar.** El programa es un solo archivo, `panostitch`
  (`panostitch.exe` en Windows), más `panostack` para apilar estrellas desde
  la línea de comandos. En Linux necesita un controlador gráfico con OpenGL y
  X11 o Wayland, como tiene cualquier escritorio.
- **Los archivos RAW de cámara** no necesitan nada más: lleva un decodificador
  RAW integrado. También se usan `dcraw_emu` de LibRaw, RawTherapee, darktable
  o dcraw si los tiene instalados (ver [Archivos RAW](#8-archivos-raw)).
- **Los archivos FITS y XISF** no necesitan nada más.
- **Los diálogos de archivos** no necesitan nada en Windows ni macOS. En Linux
  usan `zenity`, `kdialog` o `yad`, el que esté instalado (casi todos los
  escritorios tienen uno). Sin ninguno, escriba las rutas en el campo de ruta.

Para compilar el programa desde el código fuente necesita Rust
(<https://rustup.rs>): `cargo build --release` crea
`target/release/panostitch` y `target/release/panostack`.

---

## 2. Iniciar el programa

Haga doble clic en `panostitch`, o inícielo desde una terminal:

```
panostitch
```

También puede indicar fotos o un archivo de proyecto para abrirlos al momento:

```
panostitch ~/Imágenes/pano/*.NEF
panostitch mi_pano.panoproj
panostitch noche.stackproj          # se abre en la pestaña Apilado de estrellas
panostitch noche.tlproj             # se abre en la pestaña Timelapse
```

Notas:

- El programa usa los núcleos del procesador automáticamente. Los trabajos
  largos usan como máximo el 85 % y dejan libre el 15 % de la memoria, para que
  el ordenador siga siendo usable.
- La ventana se abre en uno o dos segundos; no hay que preparar nada.
- Si no aparece ninguna ventana, inicie el programa desde una terminal: allí
  se muestran los mensajes de error.
- Los interruptores de aspecto (**Oscuro**, **Moderno**, **Simple**) se guardan en
  `ui.toml` en la carpeta de ajustes: `~/.config/panostitch` en Linux,
  `~/Library/Application Support/panostitch` en macOS,
  `%APPDATA%\panostitch` en Windows.
- **Idioma:** la ventana habla español, inglés, checo, alemán, francés o
  polaco; el menú de arriba a la derecha cambia al instante (el primer inicio
  sigue el idioma del sistema).
- **Ayuda:** **? Ayuda** arriba a la derecha (o F1) abre este manual dentro del
  programa, con índice, búsqueda y enlaces; F1 abre el capítulo de la pestaña
  visible.
- **Se recuerda entre sesiones:** la pestaña en la que estaba, los ajustes del
  apilado, los ajustes de vídeo del timelapse (deflicker, movimiento de cámara,
  salida, **RAW con**) y los ajustes RAW de la unión de panorámicas. Los
  archivos, máscaras y aspectos no se recuerdan; eso lo guarda un archivo de
  proyecto.
- **Arrastrar y soltar:** suelte imágenes o una carpeta sobre la ventana para
  añadirlas a la pestaña visible (Posprocesado abre la primera imagen). Un
  archivo de proyecto soltado se abre en su propia pestaña.
- **Proyectos recientes** (barra superior) muestra los últimos proyectos que
  abrió o guardó, de todo tipo; un clic abre uno en su pestaña.
- Los diálogos de archivos empiezan en la última carpeta usada.
- Tras guardar, **Mostrar en la carpeta** (en el informe, o junto a **⬇ Guardar**
  en Posprocesado) abre la carpeta en el gestor de archivos.

---

## 3. La ventana de un vistazo

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

(El esquema muestra los nombres en inglés; en la ventana aparecen en el idioma
elegido.)

- **Columna izquierda:** abrir y guardar proyectos, elegir las imágenes, buscar
  puntos clave, emparejar las imágenes.
- **Centro:** la vista principal, con botones para cambiar de vista, hacer
  zoom, ocultar las superposiciones y deshacer o rehacer.
- **Columna derecha:** proyección, exposición, fusión, brillo, colocación
  manual, estirado del histograma, vista previa y guardado.
- **Línea de estado roja** (bajo la vista): lo que hace el programa,
  resultados y mensajes de error.

Los interruptores de arriba a la derecha cambian el aspecto: **Oscuro**,
**Moderno** (controles más redondeados y espaciosos) y **Simple**. El modo
simple oculta los controles detallados (ajustes de detección y emparejado,
fusión, estirado, desplazamiento fino) y deja lo que necesitan la mayoría de
las panorámicas: las imágenes, un preajuste, **Ejecutar todos los pasos**, la
proyección con focal, guiñada, cabeceo y alabeo, la exposición, **Igualar**, el
recorte, la vista previa y el guardado. Los ajustes ocultos conservan los
valores del preajuste. El programa recuerda los interruptores.

---

## 4. Inicio rápido

**Panorámica fotográfica:**

1. Deje activado **RAW a medio tamaño**. Es más rápido y basta para revisar el
   resultado.
2. Elija un **Preajuste** acorde a su tema (ver [Preajustes](#50-preajustes)).
3. Escriba la carpeta de sus fotos en el campo de ruta y pulse **Intro**, o haga
   clic en **Añadir…** y seleccione las fotos. **El orden no importa.**
4. Haga clic en **Ejecutar todos los pasos** y espere a que la línea de estado
   diga *Hecho*.
5. Elija una **proyección** a la derecha: **cilíndrica** para una sola fila,
   **esférica** para varias filas o lo que llegue muy arriba o abajo. Haga clic
   en **Exposición automática** si queda muy oscura, y ajuste **Cabeceo (°)** si
   el horizonte se curva. Haga clic en **Recorte automático** para quitar los
   bordes irregulares.
6. Haga clic en **⬇ Guardar panorámica…**. Haga clic en **⬇ Guardar proyecto…**
   si quiere volver a ella más tarde.

**Mosaico de cielo profundo a partir de archivos FITS:**

1. Elija el preajuste **mosaico de cielo profundo**.
2. Cargue las teselas FITS (apiladas o tomas sueltas). Se estiran
   automáticamente para verlas.
3. Haga clic en **Ejecutar todos los pasos** y luego en **Estirado automático**
   para ajustar el estirado a todo el mosaico, y retóquelo si quiere.
4. Guarde como **`.fits`** para conservar los datos lineales para procesar
   después, o como **`.tif`** para obtener la imagen estirada que ve.

---

## 5. Paso a paso

**Escribir valores:** cada deslizador tiene un campo numérico al lado. Haga
clic en el campo, escriba un valor y pulse **Intro**. El valor se ajusta al
paso y al rango del deslizador.

### 5.0 Preajustes

El menú **Preajuste** (columna izquierda, bajo **RAW con**) fija de una vez todos
los ajustes de emparejado y composición para un tipo de tema. Cada valor se
puede cambiar después.

| Preajuste | Para | Ajustes principales |
|---|---|---|
| **paisaje de día** | panorámicas diurnas, cámara girada en el sitio | poca reducción de ruido, homografía, cilíndrica |
| **astro / paisaje nocturno** | cielos nocturnos, Vía Láctea sobre un paisaje | mucha reducción de ruido, emparejado más tolerante, esférica, +1 EV (luego **Exposición automática**), umbral de esquinas más alto (menos esquinas falsas en el ruido) |
| **mosaico de cielo profundo** | mosaicos de campos estelares con telescopio, p. ej. FITS | poca reducción de ruido (las estrellas son diminutas), modelo de semejanza (desplazamiento + rotación de campo), sin refinado, rectilínea; use **Estirado automático** |
| **macro / microscopio** | un portaobjetos o una plataforma movidos bajo una cámara fija | modelo de traslación (las tomas solo se desplazan), sin refinado, rectilínea |
| **escaneo / documento plano** | mapas, cuadros, documentos fotografiados por partes | modelo afín, sin refinado, rectilínea |

### 5.1 Imágenes

| Control | Qué hace |
|---|---|
| **Añadir…** | Abre un diálogo de archivos. Puede seleccionar varios archivos (Ctrl/Mayús-clic). |
| **Campo de ruta** | Escriba la ruta de un archivo o de una **carpeta** y pulse Intro. Una carpeta carga todas sus imágenes, ordenadas por nombre. Los archivos llamados `panorama…` (resultados anteriores) se omiten. |
| **Lista de imágenes** | Haga clic en una imagen para seleccionarla y mostrarla. **Ctrl-clic** añade o quita una imagen de la selección, **Mayús-clic** selecciona un rango y **Ctrl+A** (con el ratón sobre la lista) selecciona todas. Desplácese con la rueda del ratón. |
| **Quitar** | Quita las imágenes seleccionadas. La tecla **Supr** hace lo mismo con el ratón sobre la lista. |
| **Vaciar todo** | Quita todas las imágenes. |
| **▲ / ▼** | Mueve las imágenes seleccionadas arriba o abajo en la lista. Solo los modos de pares *secuencial* y *rejilla* dependen del orden. |
| **Referencia** | La imagen alrededor de la cual se colocan las demás; queda sin deformar. **auto (centro)** elige la imagen del centro del grupo, que suele dar el resultado más uniforme. |
| **RAW con** | Qué conversor RAW usar; ver [Archivos RAW](#8-archivos-raw). Se aplica a las imágenes que se añadan después. |
| **RAW a medio tamaño** | Decodifica los RAW y los FITS de cámaras a color a media resolución: unas 4× más rápido y ¼ de la memoria. Se aplica a las imágenes que se añadan después. Los RAW se vuelven a leer a tamaño completo cuando guarda con un **Tamaño de salida** superior al 50 %. |

La lista marca cada entrada:

| Marca | Significado |
|---|---|
| línea azul | seleccionada; la más oscura es la imagen actual (para ver, brillo y mover) |
| `R` | imagen de referencia |
| `4345kp` | número de puntos clave encontrados |
| `UNLINKED` | la imagen no se pudo conectar a la panorámica (ver [5.3](#53-paso-2-emparejado)) |

Los resultados del emparejado se conservan al reordenar imágenes o quitar
otras, así que no hay que volver a emparejar.

### 5.2 Paso 1: puntos clave

Los puntos clave son lugares característicos, como esquinas, bordes y
estrellas, que se pueden reconocer en dos imágenes solapadas.

| Deslizador | Por defecto | Significado |
|---|---|---|
| **Tamaño de detección (px)** | 2000 | La imagen se reduce a este tamaño (lado mayor) antes de buscar. Más grande es más preciso pero más lento. |
| **Puntos clave máx.** | 5000 | Límite por imagen. Los puntos se reparten por toda la imagen. |
| **Umbral de esquinas** | 0.001 | Fuerza mínima de una esquina. Más bajo encuentra esquinas más débiles (útil para cielo o niebla); más alto deja solo las fuertes. |
| **Reducir ruido (σ px)** | 1.5 | Difumina el ruido del sensor antes de buscar. Súbalo (2–3) para **tomas nocturnas** o fotos ruidosas de ISO alto; bájelo (0,5–1) para campos de estrellas y fotos diurnas limpias con detalle fino. |

Haga clic en **Detectar puntos clave**. La vista muestra la imagen actual con
los puntos como **círculos amarillos**. Deben repartirse por las partes de la
imagen que se solapan con otras. Solo se vuelven a procesar las imágenes cuyos
ajustes cambiaron. Las imágenes oscuras y lineales (tomas nocturnas, FITS) se
manejan bien: la detección ajusta el contraste internamente.

### 5.3 Paso 2: emparejado

El emparejado encuentra los mismos puntos clave en dos imágenes. Después
**RANSAC** conserva solo las coincidencias que encajan en un único modelo
geométrico (los *inliers*).

**Qué pares se prueban (menú Pares):**

| Pares | Úselo cuando |
|---|---|
| **todos los pares** | Por defecto. Cada imagen se prueba con todas las demás, así que **el orden no importa** y varias filas funcionan solas. Tarda más con muchas imágenes (74 fotos ≈ 35 s). |
| **secuencial** | Una fila, imágenes en orden de toma. Cada imagen se empareja solo con la siguiente de la lista. Lo más rápido. |
| **rejilla** | Varias filas en un orden conocido: **Columnas de la rejilla** imágenes por fila, fila tras fila. Se prueban los vecinos a la derecha, debajo y en diagonal. Active **serpiente** si cada segunda fila se tomó en sentido contrario. |

**Ajustes:**

| Control | Por defecto | Significado |
|---|---|---|
| **Columnas de la rejilla** | 5 | Imágenes por fila (solo modo rejilla). |
| **Prueba de razón** | 0.8 | Cuánto mejor debe ser la mejor coincidencia que la segunda. Más bajo es más estricto (menos coincidencias, más fiables); más alto da más coincidencias. |
| **Distancia máx.** | 0.35 | Cuánto pueden diferir dos puntos clave y aun así coincidir. |
| **Tol. RANSAC (px)** | 5.0 | A qué distancia (en píxeles) puede estar una coincidencia del modelo y seguir contando como inlier. |
| **Inliers mín.** | 20 | Mínimo de inliers para que un par cuente como solape real. |
| **Modelo** | homografía | Cómo se proyecta una imagen sobre otra, ver abajo. |
| **refinar** | activado | Refinado global, ver abajo. |

**Cuándo un par cuenta como solape real:** necesita al menos *Inliers mín.*
inliers, una clara mayoría de sus coincidencias debe concordar y la
transformación debe ser plausible (no debe plegar ni estirar exageradamente la
imagen). Así se filtran coincidencias falsas entre imágenes que en realidad no
se solapan.

**Modelos:**

| Modelo | Úselo cuando |
|---|---|
| **homografía** | Panorámicas normales: cámara girada en el sitio. Por defecto. |
| **afín** | Escaneos, o fotos de un objeto plano desde lejos. |
| **semejanza** | Solo desplazar, girar y escalar uniformemente, p. ej. mosaicos de telescopio con rotación de campo. Estable con pocas coincidencias. |
| **traslación** | Solo desplazar, p. ej. un escáner o la plataforma de un microscopio. Lo más robusto, lo menos flexible. |

**Colocación y refinado global (refinar):** las imágenes se enlazan por sus
pares más fuertes, empezando por la imagen de referencia. Con **refinar**
activado (solo modelo homografía), todos los pares se resuelven juntos como
una sola cámara que gira. Así los errores pequeños se reparten en vez de
sumarse, y se cierran los bucles entre filas; esto quita casi todos los
fantasmas. También estima la distorsión de barril del objetivo (fuerte en los
gran angulares) y la corrige al renderizar. Los pares que no encajan en la
solución común se **descartan automáticamente**, porque suelen ser
coincidencias falsas o algo que se movió. La línea de estado indica cuántos se
descartaron y el **error típico por par** (1–3 px es bueno).

**Cielos nocturnos: emparejar por las estrellas.** Los puntos de esquina rara
vez emparejan tomas que solo muestran cielo: cada estrella se parece a todas
las demás. Por eso, cuando fallan los puntos de un par, se comparan sus
**estrellas**: triángulos de estrellas vecinas, por los ángulos entre ellas.
Esos ángulos no cambian al girar la cámara ni al girar el cielo, así que
emparejan incluso tomas hechas con minutos de diferencia. Esos pares llevan
*(estrellas)* en el menú **Par** y la línea de estado los cuenta. Solo se
aceptan pares con suficientes estrellas en el solapamiento y una buena parte de
ellas coincidiendo. Cuando las cámaras apuntan muy separadas (una fila hacia
arriba y otra hacia abajo), las esquinas lejanas de una foto quedan detrás de
la otra cámara; ahí solo se juzga la parte que cubren las coincidencias.

**Un zoom que se movió.** Apuntando hacia arriba o hacia abajo, el anillo del
zoom puede girar por su propio peso entre tomas. Con archivos RAW se conoce la
distancia focal de cada foto: los puntos se buscan a la misma escala en todas,
y el refinado ajusta cada distancia focal por separado. La línea de estado
dice entonces *El zoom cambió entre las tomas (15–17 mm)*.

**El cielo que gira.** El cielo gira 15° por hora, así que filas tomadas con
diez minutos de diferencia lo ven desplazado 2,5°. Con archivos RAW (guardan la
hora de cada toma), el refinado mide ese giro a partir de los pares por
estrellas junto con los pares del suelo, y las cámaras quedan bien colocadas.
La línea de estado dice *el cielo giró 14,9°/hora entre las tomas*: cerca de 15
significa que funcionó. Al componer, las estrellas de todas las fotos se
muestran como estaban en un mismo momento (la mitad de la sesión), mientras el
suelo y el resplandor del cielo se quedan donde estaban; junto a árboles y
otras siluetas cada foto queda tal como se tomó.

**Botones:**

- **Emparejar**: empareja los pares elegidos en el menú Pares (en paralelo),
  luego coloca y refina.
- **Reemparejar par**: empareja solo el par mostrado, con los ajustes actuales.
- **Descartar par**: deja de usar el par mostrado (para una coincidencia que se
  ve que está mal); la colocación se rehace sin él.
- **Ejecutar todos los pasos**: puntos clave, emparejado, colocación y vista
  previa de una vez.

La vista **Coincidencias del par** muestra dos imágenes lado a lado:

- **líneas verdes**: inliers (coincidencias correctas)
- **líneas rojas**: valores atípicos. Ocúltelos con el interruptor
  **valores atípicos**.

El menú **Par** lista los pares aceptados con su número de inliers. Los pares
marcados como *descartados* fallaron alguna comprobación; el texto bajo el
menú dice por qué.

**Cómo se ve un buen resultado:** decenas de líneas verdes o más, todas con el
mismo desplazamiento, y un error medio de 1–2 px.

**Imágenes no enlazadas:** una imagen sin ningún par aceptado con el resto se
marca `UNLINKED` en la lista y **queda fuera de la panorámica**, para que no la
estropee. Active **mostrar imágenes no enlazadas** (columna derecha) para ver
esas imágenes en una fila bajo la panorámica y colocarlas a mano con
**Ctrl+arrastrar**.

### 5.4 Paso 3: proyección

Vea [Proyecciones](#7-proyecciones) para saber cómo es cada una.

| Control | Significado |
|---|---|
| **Menú de proyección** | 14 proyecciones, ver [Proyecciones](#7-proyecciones) |
| **Focal (× auto)** | La focal se estima a partir de las coincidencias. Este deslizador la escala. Si la panorámica parece comprimida o estirada a lo ancho, pruebe valores entre 0,8 y 1,2. |
| **Guiñada (°)** | Gira toda la panorámica a izquierda o derecha: elige qué dirección queda en el centro. Útil en panorámicas de 360° (p. ej. para poner el núcleo de la Vía Láctea en el centro y no en el borde). |
| **Cabeceo (°)** | Inclina toda la panorámica arriba o abajo. Úselo para **enderezar un horizonte curvado** (por ejemplo si la cámara apuntaba hacia abajo). |
| **Alabeo (°)** | Gira toda la panorámica sobre sí misma. Úselo para nivelar un horizonte inclinado. |
| *Control fino* | Guiñada, cabeceo y alabeo tienen cada uno un deslizador, un campo numérico y **◀ ▶**. Los deslizadores de cabeceo y alabeo son finos cerca de 0° (donde suelen estar las correcciones) y gruesos más lejos. Arrastre el campo numérico despacio para 0,02° por píxel, o haga doble clic y escriba. **◀ ▶** avanzan 0,1° (con Mayús, 1°). |
| **Restablecer focal / guiñada / cabeceo / alabeo** | Devuelve estos cuatro deslizadores a sus valores por defecto. |

El título de la vista muestra el tamaño de la panorámica, la focal en píxeles,
el campo de visión de la imagen de referencia y el error típico de alineación.

Estos controles actualizan la vista previa al instante (una vez que existe).

### 5.5 Paso 4: composición

| Control | Significado |
|---|---|
| **Fusión** | **inteligente** (por defecto): fusión multibanda con eliminación de fantasmas. El detalle fino viene de una sola imagen en cada sitio, así que no hay imágenes dobles de personas, luces u objetos cercanos. Las diferencias de brillo se funden en una zona amplia, así que no hay uniones duras. Donde las imágenes solapadas no concuerdan (algo se movió), toda esa zona se toma de una sola imagen. **difuminado**: mezcla suave simple; muestra fantasmas donde algo se movió. **costura**: cada píxel de la imagen más central; sin fantasmas, pero con uniones duras. **media**: media simple; sobre todo para comprobar la alineación. |
| **Tamaño de vista previa (px)** | Tamaño de la vista previa (lado mayor). Más grande es más nítido pero más lento. Al hacer zoom se renderiza el detalle de todos modos (ver [Moverse por la vista](#6-moverse-por-la-vista)). |
| **Exposición (EV)** | Aclara u oscurece toda la panorámica, de −3 a +8 EV (+1 EV = el doble de brillo). Las panorámicas astronómicas oscuras suelen necesitar de +1,5 a +4. |
| **Brillo (sel.)** | Factor de brillo de las imágenes **seleccionadas**. |
| **mostrar imágenes no enlazadas** | Muestra también las imágenes que no se pudieron enlazar, en una fila bajo la panorámica, para colocarlas a mano. |
| **Exposición automática** | Fija la **Exposición** a partir de la vista previa actual: el brillo típico a gris medio, sin recortar más del 0,5 % más brillante (estrellas, farolas). |
| **Igualar** | Calcula factores de brillo por imagen para que los solapes coincidan. |
| **Restablecer desplazamientos** | Deshace todos los desplazamientos manuales. |
| **Recorte automático** | Recorta la panorámica al mayor rectángulo cubierto por completo por imágenes, así desaparecen los bordes negros e irregulares. La vista previa muestra entonces el resultado recortado, y **⬇ Guardar panorámica…** lo guarda. La línea de estado dice cuánto se conserva. Enderece antes el horizonte (cabeceo, alabeo); una panorámica curvada deja solo un rectángulo estrecho. |
| **Quitar recorte** | Quita el recorte. El recorte también se quita solo cuando cambia algo que mueve las imágenes (proyección, focal, guiñada, cabeceo, alabeo, desplazamientos, emparejado). |
| **Mover la seleccionada ← ↑ ↓ →** | Mueve la imagen actual un píxel de vista previa (ver abajo). |

En la vista **Panorámica** se dibuja y numera el contorno de cada imagen:

- **naranja**: la imagen actual
- **azul claro**: otras imágenes seleccionadas
- **cian**: la imagen de referencia
- **blanco**: las demás imágenes
- **líneas amarillas discontinuas**: los enlaces usados para colocar las imágenes

Desactívelos con **superposiciones** en la barra de herramientas para ver la
panorámica sola.

**Haga clic dentro de un contorno** para seleccionar esa imagen (también con
las superposiciones desactivadas).

**Mover imágenes a mano:** mantenga **Ctrl** y arrastre una imagen, o use las
flechas del teclado o los botones de desplazamiento. La imagen se mueve junto
con las imágenes unidas a ella más lejos de la referencia (por los enlaces
amarillos). La imagen de referencia no se mueve; use cabeceo y alabeo para
mover todo.

### 5.6 Paso 5: estirado

Las imágenes astronómicas (y los revelados RAW oscuros) suelen ser
**lineales**: el detalle interesante está en el pequeño porcentaje más oscuro
del rango de brillo y la imagen parece casi negra. El estirado del histograma
lo hace visible.

| Control | Significado |
|---|---|
| **Histograma** | Distribución del brillo de la vista previa de la panorámica (en la vista Panorámica) o de la imagen actual (en las otras vistas), en escala logarítmica. La **línea azul** marca el punto negro, la **línea blanca** el punto blanco, y la **curva naranja** muestra cómo el brillo de entrada (de izquierda a derecha) pasa al brillo de salida (de abajo arriba). |
| **Punto negro** | Todo lo que tenga este brillo o menos se vuelve negro. Póngalo justo bajo el fondo del cielo (el borde izquierdo del pico principal del histograma). |
| **Medios tonos** | Aclara (valores más pequeños) u oscurece (valores más grandes) los tonos medios sin recortar. 0,5 es neutro. Las imágenes de cielo profundo suelen necesitar 0,005–0,05. |
| **Punto blanco** | Todo lo que tenga este brillo o más se vuelve blanco. |
| **Estirado automático** | Fija los tres a partir del histograma actual: punto negro justo bajo el fondo del cielo, medios tonos para que el fondo quede en torno al 25 % de gris (la misma idea que el estirado automático de pantalla de PixInsight). |
| **Restablecer estirado** | Vuelve a sin estirado (0 / 0,5 / 1). |

- El estirado solo cambia cómo se **muestran** las imágenes y cómo se guarda la
  panorámica como TIFF, PNG o JPEG. El emparejado trabaja siempre con los datos
  originales.
- **Los archivos FITS** reciben un estirado automático para verlos al cargarlos,
  basado en todas las imágenes FITS cargadas juntas.
- Cambiar el estirado es instantáneo: no hay que recalcular nada.
- La **Exposición** se aplica antes del estirado.

### 5.7 Deshacer, rehacer y proyectos

**Deshacer / rehacer:** **↶ Deshacer** y **↷ Rehacer** en la barra de
herramientas, o **Ctrl+Z** y **Ctrl+Y** (también **Ctrl+Mayús+Z**). La línea de
estado dice qué se deshizo, p. ej. *«Deshecho: Cabeceo (°) 0 → 10»*.

- Se puede deshacer: cada deslizador, menú e interruptor de los ajustes;
  añadir, quitar y reordenar imágenes; los resultados del emparejado
  (Emparejar, Reemparejar, Descartar); la imagen de referencia; los
  desplazamientos manuales; el brillo; el recorte automático.
- Arrastrar un deslizador cuenta como **un** paso, por mucho que arrastre. Los
  cambios en dos controles distintos son siempre dos pasos.
- Las imágenes quitadas se guardan en memoria para deshacer, así vuelven al
  instante, junto con sus resultados de emparejado.
- Se guardan los últimos 100 pasos. No se puede deshacer: lo que solo afecta a
  la vista (zoom, qué vista se muestra, superposiciones), **RAW con** y
  **RAW a medio tamaño**, y guardar archivos.

**Proyectos:** **⬇ Guardar proyecto…** (arriba en la columna izquierda) escribe
un archivo de proyecto con la extensión **`.panoproj`**. **⇪ Abrir proyecto…**
lo vuelve a cargar. También puede escribir la ruta de un archivo `.panoproj` en
el campo de ruta y pulsar Intro, o iniciar el programa con él:
`panostitch mi_pano.panoproj`.

Un proyecto guarda:

- las imágenes (ruta completa y ruta relativa al archivo de proyecto)
- todos los ajustes: deslizadores, menús, interruptores, incluido el estirado
- los resultados del emparejado, así no hay que volver a emparejar
- los desplazamientos manuales, los factores de brillo, la imagen de
  referencia y el recorte
- **RAW con** y **RAW a medio tamaño**, para que las imágenes se carguen igual

**No** contiene las imágenes en sí; se cargan de sus archivos. Si mueve las
fotos junto con el archivo de proyecto (p. ej. toda la carpeta), se siguen
encontrando por la ruta relativa. Los archivos que falten se indican y se
omiten.

El archivo es texto legible (formato TOML). Su tamaño se debe sobre todo a los
puntos de coincidencia guardados: unos 50 kB por imagen.

---

## 6. Moverse por la vista

| Acción | Ratón | Teclado | Botón |
|---|---|---|---|
| Desplazar | arrastrar con el botón **izquierdo** | flechas (vistas Imagen y Par) | |
| Acercar | rueda hacia arriba | `+` | **Zoom +** |
| Alejar | rueda hacia abajo | `-` | **Zoom -** |
| Restablecer zoom | | `F`, `0` o `Inicio` | **Ajustar** |
| Seleccionar imagen (vista Panorámica) | clic dentro de su contorno | | |
| Mostrar / ocultar contornos, números, enlaces | | | interruptor **superposiciones** |
| Mover imagen (vista Panorámica) | **Ctrl + arrastrar** | flechas (`Mayús` = 10 pasos) | **← ↑ ↓ →** |
| Deshacer | | `Ctrl+Z` | **↶ Deshacer** |
| Rehacer | | `Ctrl+Y` o `Ctrl+Mayús+Z` | **↷ Rehacer** |

Los botones **Imagen + puntos clave**, **Coincidencias del par** y
**Panorámica** cambian entre las tres vistas.

**Otros atajos** (pase el ratón sobre un botón para ver su atajo):

| Teclas | Unión de panorámicas | Apilado de estrellas |
|---|---|---|
| `Ctrl+Intro` | **▶ Ejecutar todos los pasos** | **▶ Iniciar** |
| `P` / `Ctrl+P` | `P`: **◐ Vista previa de la panorámica** | `Ctrl+P`: **◐ Vista previa rápida** |
| `Ctrl+S` | guardar el proyecto (directamente en el archivo abierto; la primera vez pide un nombre) | igual |
| `Ctrl+Mayús+S` | **⬇ Guardar panorámica…** | |
| `Ctrl+O` | **⇪ Abrir proyecto…** | igual |
| `Esc` | | cancelar el apilado en curso |
| `F` | ajustar la imagen a la vista | igual |
| `F1` | este manual, en el capítulo de la pestaña | igual |

En macOS use `Cmd` en lugar de `Ctrl`.

**Cerrar la ventana** pregunta antes si hay un apilado en marcha o si el
trabajo de una pestaña no está guardado en un archivo de proyecto. Puede
entonces guardar el proyecto, salir de todos modos o cancelar.

**Zoom nítido:** la vista previa es una imagen reducida. Cuando acerca la
panorámica y se detiene un momento, la zona visible se vuelve a renderizar a
resolución de pantalla desde las imágenes originales, para revisar detalle fino
y uniones.

---

## 7. Proyecciones

Una panorámica abarca más de lo que puede mostrar una foto plana, así que el
programa debe *proyectar* la vista sobre una imagen plana. Cada proyección
deforma algo:

| Proyección | Aspecto | Ideal para | Límites |
|---|---|---|---|
| **rectilínea** | una foto normal: las rectas siguen rectas | panorámicas estrechas (hasta ~100°), arquitectura, mosaicos de telescopio | los bordes se estiran muchísimo al ensanchar la vista |
| **cilíndrica** | las rectas horizontales se curvan, las verticales siguen verticales | la mayoría de panorámicas horizontales | no para vistas altas (arriba y abajo se estiran) |
| **esférica** (equirrectangular) | como la cilíndrica, con menos estiramiento vertical | panorámicas muy anchas o de 360°, visores de panorámicas | las rectas se curvan |
| **Mercator** | entre cilíndrica y esférica | panorámicas anchas con algo de altura | estira cerca de arriba y abajo |
| **estereográfica** | como un ojo de pez, mantiene las formas redondas | campos de visión muy amplios, tomas creativas | curvatura fuerte |
| **cilíndrica transversal** | la cilíndrica puesta de lado: las verticales se curvan, las horizontales siguen rectas | **panorámicas verticales** (del horizonte al cenit), el arco de la Vía Láctea en vertical | no para barridos horizontales amplios (se envuelven por arriba y abajo) |
| **Mercator transversal** | como la cilíndrica transversal, menos estirada a los lados | panorámicas verticales con algo de anchura | como arriba |
| **ojo de pez** (equidistante) | imagen redonda, distancia al centro proporcional al ángulo | **vistas de todo el cielo**: incline con Cabeceo para que el cenit quede en el centro | bordes comprimidos |
| **ojo de pez equisólido** | imagen redonda que mantiene las áreas en proporción | vistas de todo el cielo, como se ven la mayoría de ojos de pez | bordes aún más comprimidos |
| **pequeño planeta** | el suelo en el centro, el cielo alrededor como un anillo | panorámicas de 360° con suelo, creativo | necesita una panorámica que llegue muy abajo |
| **túnel** | el cielo en el centro, el horizonte alrededor | cielos nocturnos de 360°, trazas estelares, árboles desde abajo | necesita una panorámica que llegue muy arriba |
| **ortográfica** | un globo visto desde fuera | creativo; muestra media esfera | los bordes muy comprimidos |
| **Hammer** (Hammer–Aitoff) | un mapa ovalado de toda la esfera con áreas relativas correctas | panorámicas de esfera completa en una imagen, mapas estelares de todo el cielo | formas deformadas cerca de los bordes |
| **Mollweide** | un mapa ovalado de igual área, más plano en los polos | como Hammer; el mapa clásico de todo el cielo en astronomía | formas deformadas cerca de los bordes |

Si el horizonte parece una sonrisa o un ceño, ajuste **Cabeceo (°)**. Si está
inclinado, ajuste **Alabeo (°)**. **Guiñada (°)** gira la vista hacia los lados.
En las vistas de ojo de pez, pequeño planeta y túnel, el cabeceo decide qué
queda en el centro.

---

## 8. Archivos RAW

Compatibles: **Canon CR2 y CR3, Nikon NEF y NRW, Sony ARW y SRF, DNG, Fuji
RAF, Olympus ORF, Panasonic RW2, Pentax PEF, Samsung SRW**, y otros. La
compatibilidad exacta de cámaras depende del decodificador; el integrado
conoce la mayoría de cámaras de las dos últimas décadas, LibRaw (si está
instalado) unas 1250.

**RAW con** elige el decodificador:

| Opción | Decodificador | Carácter |
|---|---|---|
| **auto** | el primero que funcione, en el orden de abajo; RawTherapee primero si la foto tiene un archivo `.pp3` | recomendado |
| **built-in** | el decodificador propio del programa | siempre disponible, rápido; revelado neutro con los colores y el balance de blancos de la cámara y sin aclarado automático, así todas las tomas mantienen la misma exposición |
| **libraw** | `dcraw_emu` (LibRaw) | parecido al integrado; para cámaras que el integrado no conoce |
| **rawtherapee** | `rawtherapee-cli` | **usa sus ediciones de RawTherapee** guardadas en `foto.NEF.pp3`; si no, el aspecto por defecto de RawTherapee |
| **darktable** | `darktable-cli` | el revelado por defecto de darktable (con corrección de objetivo si la hay); el más lento |
| **dcraw** | `dcraw` | de reserva; no lee CR3 |

Solo el integrado está siempre disponible; los demás aparecen cuando el
programa correspondiente está instalado en su ordenador. El apilado de
estrellas necesita datos lineales, así que solo usa built-in, libraw o dcraw.

Los decodificadores externos escriben una imagen de 16 bits en una carpeta
temporal que se borra después. No se escribe nada junto a sus fotos.

**Consejo:** para colores coherentes en toda la panorámica, use las mismas
ediciones (el mismo perfil `.pp3`) para todas las tomas, o use **built-in** y
corrija los colores después.

---

## 9. Archivos FITS (astronomía)

Los archivos que terminan en **`.fits`, `.fit` o `.fts`** se leen directamente,
sin software adicional. Compatibles:

| Contenido | Ejemplos | Cómo se leen |
|---|---|---|
| **Enteros de 8, 16, 32 y 64 bits, coma flotante de 32 y 64 bits** | capturas de cámara, resultados de apilado | escalados a 0…1 (16 bits sin signo: 0…65535) |
| **Color (3 planos)** | resultados apilados de Siril, PixInsight, DSS | como RGB |
| **Mono** | cámaras monocromas, filtros sueltos | como gris |
| **RAW de cámara a color (Bayer)** | tomas de ZWO, ToupTek, QHY… con cabecera `BAYERPAT` (RGGB, BGGR, GRBG, GBRG) | debayerizadas; con **RAW a medio tamaño** un píxel de color por celda de 2×2 (rápido, limpio), si no interpoladas a tamaño completo |
| **Orden de filas** | `ROWORDER` = `TOP-DOWN` o `BOTTOM-UP` (lo habitual en FITS) | la imagen se muestra del derecho |

No compatibles: **archivos comprimidos por teselas (`.fz`)**. Descomprímalos
antes, p. ej. con `funpack`. Las partes extra de un archivo (tablas, perfiles
ICC) se ignoran; se usa la primera imagen.

**Notas para mosaicos de cielo profundo:**

- Use el preajuste **mosaico de cielo profundo**. Las teselas de telescopio
  solo se desplazan y giran (rotación de campo), que es exactamente lo que
  describe el modelo de *semejanza*.
- **Las teselas apiladas y calibradas** se unen mejor. Las tomas cortas
  sueltas tienen pocas estrellas visibles y mucho ruido.
- Las tomas RAW de cámaras a color no tienen balance de blancos, así que se ven
  verdes; el estirado no cambia el balance de color.
- Guarde como **FITS** para seguir procesando (extracción de fondo,
  calibración de color, estirado final) en Siril o PixInsight. La salida FITS
  conserva los valores lineales.

---

## 10. Guardar

Haga clic en **⬇ Guardar panorámica…** y elija un nombre de archivo. El tipo
de archivo sigue la extensión:

| Extensión | Resultado |
|---|---|
| `.tif` / `.tiff` | 16 bits por canal, lo mejor para editar después; se guarda **tal como se ve** (con exposición y estirado) |
| `.png` | 16 bits por canal; tal como se ve |
| `.jpg` | 8 bits por canal, archivo más pequeño; tal como se ve |
| `.fits` / `.fit` / `.fts` | coma flotante de 32 bits, tres planos de color; **lineal** (con exposición y factores de brillo, pero **sin estirado** ni recorte), para software de astronomía |

Cuando todas las imágenes cargadas son FITS, el nombre de archivo sugerido
termina en `.fits`.

**Tamaño de salida** (sobre el botón de guardar) fija el tamaño de la
panorámica guardada, en % de la resolución completa de la cámara; la línea de
debajo muestra el tamaño en píxeles (tras una vista previa). Los botones ¼, ½,
completo y 2× son atajos. El 100 % conserva todo el detalle; los RAW cargados
con **RAW a medio tamaño** se vuelven a leer a tamaño completo al guardar, lo
que lleva más tiempo y memoria. El 50 % o el 25 % dan un archivo más pequeño
rápidamente; por encima del 100 % se amplía (sin detalle nuevo).

Bajo el tamaño, una estimación muestra cuánta memoria necesita el guardado y
cuánto puede ocupar el archivo, junto a lo que hay libre. Se vuelve naranja
(⚠) cuando no cabe; pase el ratón por encima para ver qué hacer. Entonces el
guardado se niega con el mismo consejo en lugar de llenar la memoria, lo que
haría que todo el ordenador dejara de responder. La carga se detiene antes, con
un mensaje, cuando la memoria está casi llena.

Una panorámica grande se renderiza en franjas de filas cuando no cabe de una
vez en la memoria; la línea de estado cuenta entonces las franjas. Solo la
panorámica terminada se guarda entera, en el formato del archivo (3 bytes por
píxel para JPEG, 6 para TIFF y PNG de 16 bits), y las fotos leídas de nuevo a
tamaño completo se guardan en media precisión. Así, un TIFF de 16 bits de
20 000 × 8000 px necesita alrededor de 1 GB más una franja y las fotos, donde
antes necesitaba unos 8 GB. Con la fusión **inteligente** cada franja se
renderiza con algo de solape, así que las franjas se unen sin un escalón
visible. El guardado nunca usa más del 85 % de la memoria.

Se rechazan las panorámicas de más de 60 000 px de ancho o alto. Casi siempre
significa que un par está mal emparejado o que la proyección no es adecuada;
pruebe cilíndrica o esférica.

Mientras la unión trabaja, la línea de estado muestra el tiempo transcurrido.
Tras guardar, un pequeño **informe** bajo el botón de guardar lo resume:
cuántas fotos hay en la panorámica (y cuántas no emparejaron), el tiempo de
unión (carga, emparejado y vistas previas desde que se añadió la primera foto),
el tiempo de guardado, el tamaño en píxeles y el archivo con su tamaño, o el
error si el guardado falló. × lo cierra.

---

## 11. Solución de problemas

| Problema | Qué probar |
|---|---|
| **No aparece ninguna ventana** | En Windows un mensaje dice por qué; en Linux y macOS inicie el programa desde una terminal y lea allí el error. La ventana necesita OpenGL 2.0 o más reciente: actualice el controlador gráfico (en PC antiguos, el del fabricante del chip gráfico). |
| **El programa se cerró inesperadamente** | En el siguiente inicio la línea de estado lo indica. Los detalles están en `crash.log` en la carpeta de ajustes (Linux `~/.config/panostitch`, Windows `%APPDATA%\panostitch`, macOS `~/Library/Application Support/panostitch`); adjúntelo al informar del problema. |
| **Controles cortados en el lateral** | Arrastre el borde entre un panel lateral y la imagen para ensancharlo. |
| **«Ocupado – espere…»** | Todavía se está ejecutando un paso; espere a que cambie la línea de estado. |
| **Una imagen se omite al cargar** | La línea de estado dice qué archivo y por qué. Para RAW, pruebe otro conversor en **RAW con**. Para FITS `.fz`, descomprima antes el archivo. |
| **La panorámica es casi negra** | Fotos: haga clic en **Exposición automática** o suba **Exposición (EV)** (las tomas nocturnas suelen necesitar de +1,5 a +4), o edite las fotos en RawTherapee (se usan entonces sus archivos `.pp3`). FITS y otros datos lineales: haga clic en **Estirado automático**. |
| **Las imágenes FITS se ven negras o blancas al cargar** | El estirado automático al cargar se basa en todas las imágenes FITS; una toma muy sobre- o subexpuesta puede desajustarlo. Quite esas tomas y haga clic en **Estirado automático** (o **Restablecer estirado** y ajústelo a mano). |
| **Las imágenes FITS se ven verdes** | Las tomas RAW de cámaras a color no tienen balance de blancos. Apílelas y calibre el color antes, o corrija el color del resultado en software de astronomía. |
| **Pocos puntos clave amarillos** | Baje el **Umbral de esquinas**; suba **Puntos clave máx.**; suba **Tamaño de detección (px)**. |
| **Puntos clave sobre todo en hierba, cielo o ruido** | Suba **Reducir ruido (σ px)** a 2–3. |
| **Imágenes marcadas UNLINKED** | Mírelas: ¿se solapan de verdad con otra imagen? Suba **Prueba de razón** (0,85–0,9) y **Distancia máx.** (0,4), baje **Inliers mín.**, suba **Reducir ruido (σ px)** y pulse **Emparejar** otra vez. Si sigue fallando, active **mostrar imágenes no enlazadas** y colóquelas con **Ctrl+arrastrar**. |
| **Un agujero en el cenit, o tomas solo de cielo sin enlazar (UNLINKED)** | Use los archivos RAW (se necesitan su distancia focal y su hora), mantenga **todos los pares** y pulse **Emparejar** de nuevo: las tomas solo de cielo se emparejan entonces por sus estrellas. Cada toma de cielo necesita unas decenas de estrellas y un buen solapamiento con sus vecinas. |
| **Filas hacia abajo (o el nadir) sin enlazar** | El suelo fotografiado de cerca se desplaza con el menor movimiento de la cámara (paralaje), y la hierba de noche es sobre todo ruido. Haga las filas del suelo con un cabezal panorámico que gire alrededor del objetivo, o tape el nadir después. |
| **Nubes, agua u otras cosas que se mueven** | Cambian entre tomas, así que no se pueden emparejar de forma fiable (las tomas con solo nubes suelen quedar sin enlazar). Haga esas partes rápido, una tras otra, o colóquelas a mano. |
| **Se usa un par erróneo** (líneas en todas direcciones, imágenes en el sitio equivocado) | Selecciónelo en el menú **Par** y haga clic en **Descartar par**. Con **refinar** activado, la mayoría de esos pares se descartan solos. |
| **Fantasmas (objetos duplicados)** | Mantenga **refinar** activado y use **Fusión → inteligente** (por defecto). Los problemas restantes vienen de cosas que cambiaron entre tomas (p. ej. una linterna encendida en una sola foto), o del **paralaje**: los objetos cercanos se desplazan contra el fondo cuando la cámara no gira alrededor de su objetivo (el «punto nodal»). |
| **Escalones de brillo visibles** | **Igualar**, y luego afinar por imagen con **Brillo (sel.)**. |
| **Horizonte curvado** | Ajuste **Cabeceo (°)** (y **Alabeo (°)** si está inclinado). |
| **Arriba o abajo muy estirado** | Use la proyección **esférica**; la línea de estado la sugiere cuando la panorámica llega muy arriba o abajo. |
| **Panorámica comprimida o estirada** | Ajuste **Focal (× auto)**. |
| **Panorámica enorme y deformada** | Un par erróneo (revise los pares), o use **esférica** / **cilíndrica** en vez de rectilínea. |
| **Lento o sin memoria** | Active **RAW a medio tamaño** antes de cargar; baje **Tamaño de vista previa (px)**. Las imágenes a resolución completa necesitan unos 12 bytes por píxel cada una, unos 300 MB para una imagen de 24 MP. Con muchas imágenes, el emparejado de *todos los pares* tarda más; *rejilla* es más rápido si conoce la disposición. |
| **El recorte automático deja solo una franja fina** | La panorámica está curvada. Enderécela antes con **Cabeceo (°)** y **Alabeo (°)** (y elija una proyección adecuada), luego **Recorte automático** otra vez. |
| **Un proyecto se abre sin algunas imágenes** | La línea de estado lista los archivos que faltan. Se movieron o renombraron, y no junto con el archivo de proyecto. Devuélvalos a su sitio, o añádalos de nuevo y empareje. |
| **Deshacer no hace nada** | Deshacer trabaja con cambios asentados; espere un momento tras el último cambio. Mientras se ejecuta un paso (*Ocupado*), deshacer espera. |
| **Todo va lento** | Active **RAW a medio tamaño** antes de cargar (RAW y FITS de cámaras a color); baje **Tamaño de vista previa (px)**. En la pestaña de apilado, **Medio tamaño** en Conversión RAW. |

---

## 12. Consejos para buenas panorámicas

- **Solape** las tomas vecinas un 30–50 %, también entre filas. Para mosaicos
  de telescopio suele bastar un 15–25 %.
- **Gire la cámara** alrededor del objetivo, no alrededor de usted. Use un
  trípode si puede, idealmente con una rótula panorámica.
- **Bloquee la exposición, el balance de blancos y el enfoque** (modo manual),
  para que todas las tomas coincidan.
- **Sujete el anillo del zoom con cinta** en panorámicas nocturnas: apuntando
  hacia arriba, un zoom puede desplazarse. PanoStitch lo resuelve, pero una
  focal fija empareja mejor.
- Evite objetos en movimiento en las zonas de solape, o captúrelos en una sola
  toma.
- **Mantenga la cámara nivelada.** Una cámara inclinada curva el horizonte
  (se corrige con cabeceo y alabeo).
- En series largas, revise primero el resultado con **RAW a medio tamaño**
  activado.
- No guarde el resultado en la carpeta de las imágenes de origen si carga las
  imágenes por carpeta. Los archivos llamados `panorama…` se omiten, pero otros
  nombres no.

---

## 13. Apilado de estrellas

La segunda pestaña, **Apilado de estrellas (paisaje / trazas)**, combina muchas
exposiciones de la misma escena en una sola imagen: un cielo con poco ruido y
estrellas nítidas, o trazas estelares. Está hecha para la astrofotografía de
paisaje: la cámara está en un trípode, el cielo se mueve entre tomas, el primer
plano no.

Funciona como Sequator, con lo que a Sequator le falta: tomas bias y flat dark,
salida lineal de 32 bits, una imagen de primer plano aparte, trazas estelares
sin huecos, detección automática del cielo y nada de caja negra: cada toma se
mide y puntúa, un gráfico muestra las medidas y un registro lista cada paso con
lo que hizo y cuánto tardó.

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

**Haga clic en una entrada** de la lista de entradas o de la lista de ajustes:
sus opciones y una explicación aparecen bajo las listas.

**Los preajustes** (sobre la lista de ajustes) lo fijan todo para un tema
típico con un clic: **Paisaje con Vía Láctea** (congelar suelo, cielo
detectado automáticamente, corrección de distorsión, contaminación lumínica),
**Trazas estelares** (huecos rellenados) y **Cielo profundo (telescopio)** (todo
cielo, extracción de fondo, calibración de color, estirado automático). Cada
ajuste se puede seguir cambiando. Durante una ejecución, una barra de progreso
bajo la vista muestra el paso, el tiempo transcurrido y el tiempo restante
estimado. Al terminar, aparece un pequeño **informe** arriba del panel
derecho: cuántas tomas se apilaron de cuántas (contando las descartadas y
excluidas), las tomas de calibración, cuánto tardó, el tamaño del resultado y
el archivo guardado con su tamaño, o qué salió mal. × lo cierra.

### 13.1 Inicio rápido

1. Resalte **Lights**, haga clic en **Añadir…** y elija las tomas de estrellas
   (o escriba la ruta de un archivo o carpeta en el campo de ruta y pulse
   Intro).
2. Resalte **Región del cielo**, elija **Línea límite** y arrastre una línea a
   lo largo del horizonte en la vista de la toma base, o haga clic en
   **◎ Detectar cielo** y corrija la máscara con el pincel. Las estrellas solo
   se buscan en el cielo.
3. Haga clic en **◐ Vista previa rápida**: un resultado pequeño y rápido de
   hasta 10 tomas, para comprobar los ajustes. No se guarda nada.
4. Revise la fila **Guardar como** bajo la lista de entradas: escriba un nombre
   de archivo o elija uno de los nombres sugeridos en la lista de al lado,
   elija el tipo de archivo en la segunda lista y la carpeta con **Carpeta…**
   (por defecto: `stacked.tif` junto a la primera toma de estrellas). Si el
   archivo ya existe, la fila lo dice; **siguiente nombre libre** añade un
   número. Debajo, una estimación muestra cuánto espacio necesitan las tomas
   durante el apilado (según su número y tamaño) y si se guardan en memoria o
   en disco; se vuelve naranja (⚠) si no hay suficiente. Luego haga clic en
   **▶ Iniciar**.
5. Ajuste el posprocesado a la derecha; el resultado se actualiza al momento.
   Haga clic en **⬇ Guardar resultado…** para guardarlo de nuevo con los
   ajustes nuevos.

### 13.2 Archivos de entrada

| Entrada | Qué fotografiar |
|---|---|
| **Lights** (*Imágenes de estrellas* en modo simple) | Las tomas del cielo: misma focal, apertura y exposición. Lo mejor es RAW; TIFF, FITS o XISF solo si los datos siguen siendo lineales (sin editar). **Unificar exposición** iguala tomas hechas en condiciones distintas. |
| **Toma base** (*Imagen base* en modo simple) | La toma a la que se alinean las demás. Vacía = el light del medio. Elija otra en la lista junto a **Toma base**, o elija un light en su lista y haga clic en **Usar como base**. |
| **Darks** (*Imágenes de ruido* en modo simple) | Tomas oscuras: tapa puesta, mismo ISO y tiempo de exposición, hechas justo antes o después de las tomas de estrellas. Quitan el ruido térmico y los píxeles calientes. |
| **Tomas bias** (*Imágenes bias* en modo simple) | Exposición más corta, tapa puesta, mismo ISO. |
| **Flats** (*Imágenes de viñeteo* en modo simple) | Tomas planas: una superficie blanca iluminada de forma uniforme, misma focal y apertura. Corrigen esquinas más oscuras y sombras de polvo. |
| **Flat darks** (*Imágenes flat dark* en modo simple) | Darks para los flats: mismo ISO y tiempo de exposición que los flats. |
| **Imagen de primer plano** | Opcional: una toma del suelo desde la misma posición del trípode – en la hora azul, con exposición larga o ISO bajo. Sustituye el primer plano apilado (ruidoso, oscuro) bajo la región del cielo. **Igualar brillo al suelo apilado** la ajusta al suelo apilado; **Exposición (EV)** la aclara u oscurece a partir de ahí. Necesita una región del cielo. |
| **Salida** | Se fija en la fila **Guardar como**: nombre, tipo y carpeta. `.tif` / `.png` (16 bits), `.jpg` (8 bits), o `.fits` / `.xisf` (coma flotante de 32 bits, lineal, para procesar después en Siril, PixInsight, etc.). |

Todas las tomas de calibración son opcionales. Incluso unos pocos darks ayudan
mucho a ISO alto.

**Las listas de archivos:** cada grupo muestra sus archivos en una lista junto
a su nombre, así cientos de lights no necesitan desplazamiento. Elija ahí un
archivo para verlo en la vista previa; **Quitar** y **Usar como base** actúan
sobre ese archivo, **Vaciar** vacía todo el grupo.

**Los flats ruidosos** (demasiado pocos, o demasiado oscuros en un color, p. ej.
con una fuente de luz azul) se suavizan automáticamente; el registro lo indica,
y cuánto ruido tenían. Si no, su ruido se estamparía en cada toma y se
convertiría en finas rayas al desplazarse las estrellas.

**Archivos temporales:** no se escriben imágenes intermedias. Las tomas se
guardan en memoria o, si ocupan más de la mitad de la memoria libre, en un
archivo temporal en la carpeta de salida, que se borra en cuanto termina la
ejecución (ni siquiera es visible mientras corre). El registro dice cuál de los
dos. Si no hay sitio suficiente, la ejecución se detiene con un mensaje antes
de empezar: active **Medio tamaño** (una cuarta parte del espacio) o guarde la
salida en un disco con más espacio libre. Los masters (dark, bias, flat)
tampoco se guardan.

### 13.3 Ajustes
| Ajuste | Qué hace |
|---|---|
| **Composición** | **Alinear estrellas**: las tomas se alinean con las estrellas y se combinan. **Acumulación** las promedia; **Seleccionar mejores píxeles** (por defecto) quita los valores atípicos (aviones, satélites); **Congelar suelo** alinea solo el cielo, así cielo y primer plano quedan nítidos (necesita una región del cielo); **Solo alinear** solo escribe las tomas alineadas. **Trazas estelares**: sin alinear, el cielo conserva el valor más brillante de cada píxel. **Desvanecer trazas**: **Aparición** (las trazas empiezan débiles, el efecto de movimiento de Sequator), **Desvanecimiento** o **Ambos extremos** (afinado como un cometa). **Rellenar huecos entre tomas**: las pausas cortas del intervalo de la cámara se rellenan siguiendo el movimiento medido de las estrellas, así las trazas son líneas continuas en vez de rayas. |
| **Región del cielo** | Dónde está el cielo. **Todo cielo**: toda la toma es cielo (sin primer plano, p. ej. tomas con telescopio o de cielo profundo). **Línea límite**: arrastre una línea a lo largo del horizonte. **Degradado**: la línea más una transición suave. **Máscara irregular**: pinte el cielo con el botón izquierdo, borre con el derecho; **Tamaño del pincel** (deslizador o rueda). **◎ Detectar cielo** pinta una primera máscara a partir del brillo y las estrellas – aproximada, así que revísela y corríjala con el pincel. **Difuminado de máscara** suaviza el borde de la máscara, solo dentro del cielo, para que el primer plano no reciba nada del cielo (sin halos en el horizonte). **Mostrar estrellas detectadas** marca con círculos las estrellas que la última ejecución encontró en la toma mostrada. |
| **Sigma clipping** | Con qué rigor se quitan los valores atípicos (bajo / alto × σ). Lo habitual es 2–3; más bajo quita más pero deja más ruido. Necesita al menos 4 tomas, va mejor con 10+. |
| **Descarte de tomas** | Deja fuera las tomas claramente peores que la típica: nubes o rocío (estrellas más débiles), pocas estrellas, estrellas borrosas o alargadas (enfoque, viento, trípode golpeado). |
| **Conversión RAW** | Balance de blancos (cámara, media de la imagen, luz de día), interpolación cromática (AHD por defecto; DHT o AAHD suelen dar menos ruido de color a ISO alto) y **Medio tamaño** (4× menos píxeles, mucho más rápido). |
| **Reducir ruido dinámico** | Encuentra y quita píxeles calientes sin darks. |
| **Reducir efectos de distorsión** | Corrige la distorsión del objetivo que deja trazas cortas en las esquinas de secuencias largas: **Objetivo**, **Compleja** (gran angulares extremos, refracción cerca del horizonte) o **Auto**. |
| **Unir 4 píxeles** | Suma píxeles de 2×2: más brillo y menos ruido, mitad de ancho y alto. |
| **Timelapse** | Apila cada N tomas consecutivas en una, dando una secuencia numerada de tomas con poco ruido en una carpeta junto a la salida. |

### 13.4 Las vistas

- **Toma base**: la toma a la que se alinean las demás; aquí se dibuja la
  región del cielo. Elija un archivo en la lista de un grupo para verlo.
- **Resultado**: el apilado tras el posprocesado. Arrastre para desplazar,
  rueda para el zoom, **Ajustar** para verlo entero.
- **Informe de tomas**: un gráfico de cada medida por toma, cada una dividida
  por su mediana, para que destaque una toma mala (rojo = descartada, azul =
  la toma base); debajo, una línea por toma de estrellas: estrellas
  encontradas, tamaño de estrella (FWHM), elongación, nivel del cielo, luz
  (brillo de las estrellas frente a la toma base), error de alineación, si la
  toma se usó y por qué no. Bajo la tabla, el **registro del proceso**: cada
  paso de la ejecución, lo que hizo (calibración, región del cielo, tomas
  usadas, distorsión, relleno de huecos, primer plano, posprocesado) y cuánto
  tardó. **Haga clic en una toma** del gráfico o de la tabla para excluirla (o
  volver a incluirla), y luego otra vez en **▶ Iniciar**.

### 13.5 Posprocesado

Se aplica al resultado apilado al instante; **▶ Iniciar** guarda con estos
ajustes, **⬇ Guardar resultado…** guarda de nuevo.

| Ajuste | Qué hace |
|---|---|
| **Extracción de fondo** | Resta un modelo suave del fondo del cielo (degradados de contaminación lumínica o luz de luna). |
| **Calibración de color** | Deja el fondo del cielo gris neutro y la estrella media blanca. |
| **Reducir ruido de color** | Suaviza solo los colores, quitando las motas de color de las tomas a ISO alto. |
| **Contaminación lumínica** | **Cielo profundo**: quita un resplandor suave. **Irregular**: sigue el resplandor celda a celda (cúpulas de luz de ciudades en tomas gran angulares). **Fuerza** fija cuánto se quita. Corrija antes el viñeteo (flats), o empeorará. |
| **Realzar la luz de las estrellas** | Eleva las estrellas sobre el fondo del cielo sin aumentar su ruido. |
| **Brillo automático** | Pone el cielo en un nivel agradable sin quemar las estrellas más brillantes. |
| **Alto rango dinámico** | Comprime las estrellas brillantes y las luces en vez de recortarlas. |
| **Tono** | **curva sRGB** (como hace una cámara), **Estirado** (punto negro, medios tonos, punto blanco; automático o a mano) o **Lineal** (sin curva). |
| **Espacio de color** | sRGB o Adobe RGB para la salida TIFF y JPEG. |
| **Lineal (para procesar)** | Lo desactiva todo y guarda un FITS lineal de 32 bits: lo que esperan Siril o PixInsight. |
| **⇄ Usar en la unión de panorámicas (panel)** | Añade el resultado guardado a la pestaña Unión de panorámicas, como un panel de una panorámica (ver 13.7). |

### 13.6 Proyectos

**⬇ Guardar proyecto…** escribe un archivo `.stackproj` con las listas de
archivos, la región del cielo y cada ajuste. **⇪ Abrir proyecto…** (o
`panostitch noche.stackproj`) lo recupera. Los archivos se encuentran por su
ruta completa, o de forma relativa al archivo de proyecto si se movió la
carpeta.

### 13.7 Panorámicas de paneles apilados

Una panorámica de la Vía Láctea con poco ruido: haga varias tomas en cada
posición de la panorámica (con solape entre posiciones) y luego:

1. Apile las tomas de la primera posición (Iniciar) y haga clic en
   **⇄ Usar en la unión de panorámicas (panel)**.
2. Sustituya las tomas de estrellas por las de la siguiente posición, dé un
   nombre nuevo a la salida, Iniciar, otra vez **⇄ Usar en la unión de
   panorámicas (panel)**; y así sucesivamente.
3. En la pestaña Unión de panorámicas, **▶ Ejecutar todos los pasos** une los
   paneles apilados.

Guarde cada panel como `.tif`; el mismo posprocesado para todos los paneles
mantiene iguales su brillo y sus colores.

### 13.8 Consejos

- Haga al menos 10–20 tomas de estrellas con los mismos ajustes, y 10–20
  darks justo después, sin mover la cámara.
- Fije la región del cielo con cuidado: los árboles y edificios contados como
  cielo se emborronan con la alineación. **◎ Detectar cielo** es un buen
  comienzo, no la última palabra.
- Mire el Informe de tomas tras una vista previa rápida: el gráfico muestra
  enseguida qué tomas son peores (nubes, viento, rocío).
- Use **Congelar suelo** cuando el primer plano importe, **Seleccionar mejores
  píxeles** cuando no.
- Para procesar en otros programas, guarde como `.fits` y active **Lineal
  (para procesar)**.
- Desde una terminal, `panostack` apila sin la ventana; `panostack --help`
  lista sus opciones (ver [Uso sin la ventana](#14-uso-sin-la-ventana)).

---

## 14. Uso sin la ventana

**Apilado de estrellas desde una terminal:** `panostack` hace el mismo apilado
que la pestaña Apilado de estrellas. `panostack --help` lista todas las
opciones. Ejemplos:

```
panostack --mode freeze --horizon 0.62 --darks darks/ -o noche.tif lights/
panostack --mode trails --fade in --fill-gaps --auto-sky -o trazas.jpg lights/
panostack --preview lights/            # comprobación rápida, no se guarda nada
```

El informe de tomas y el registro del proceso se imprimen al final.

**Unir desde su propio programa en Rust:** añada PanoStitch como dependencia
(`panostitch = { path = "…/PanoStitch" }` en su `Cargo.toml`) y luego:

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

`examples/scripted.rs` es un ejemplo completo
(`cargo run --release --example scripted -- a.NEF b.NEF c.NEF`).
`stitch_files(paths, params, raw_half, log)` carga y une;
`stitch(images, params, log)` une imágenes que ya están en memoria.
Los campos de `StitchParams`, con sus valores por defecto:

| Campo | Por defecto | Significado |
|---|---|---|
| `layout` | Layout::All | qué pares emparejar: `All`, `Sequential`, `Grid` |
| `cols`, `snake` | 0 (todas las imágenes), `false` | disposición en rejilla |
| `refine` | `true` | refinado global |
| `feature_scale` | `0.5` | escala de detección |
| `nkeypoints` | `5000` |  |
| `denoise` | `1.5` |  |
| `ratio` | `0.8` |  |
| `max_distance` | `0.35` |  |
| `model` | `Model::Homography` | `Homography`, `Affine`, `Similarity`, `Translation` |
| `ransac_thresh` | `5.0` |  |
| `min_inliers` | `20` |  |
| `projection` | `Projection::Cylindrical` | cualquiera de las [Proyecciones](#7-proyecciones), p. ej. `Fisheye`, `LittlePlanet`, `Mollweide` |
| `blend` | `Blend::Smart` | `Smart`, `Feather`, `Seam`, `Average` |
| `sharpness` | `3.0` | modo difuminado: más alto = transiciones más estrechas |

**Mosaico FITS con un TIFF estirado y un resultado FITS lineal:**

```rust
use panostitch::stitcher::{Model, Projection, StitchParams};

let p = StitchParams { model: Model::Similarity, refine: false,
                       projection: Projection::Rectilinear, denoise: 0.75, ..Default::default() };
let mosaic = stitcher::stitch_files(&tiles, &p, false, &mut |_| {})?;
io::save_panorama("mosaic.fits", &mosaic)?;                          // lineal
let (black, mid, white) = stitcher::auto_stretch(&mosaic, 0.25, 2.8);
io::save_panorama("mosaic.tif", &stitcher::hist_stretch(&mosaic, black, mid, white))?;
```

Apilado de estrellas desde código: rellene un `stacker::StackConfig` (partiendo
de `StackConfig::default()`) y llame a
`stacker::stack(&config, false, &progress, None)`.

---

## 15. Archivos

| Archivo | Para qué sirve |
|---|---|
| `panostitch` (`panostitch.exe`) | El programa (la ventana). |
| `panostack` (`panostack.exe`) | Apilado de estrellas desde la línea de comandos (`panostack --help`). |
| `*.panoproj`, `*.stackproj`, `*.tlproj` | Sus archivos de proyecto (donde los guarde). |
| `ui.toml` en la carpeta de ajustes | Los interruptores de aspecto y el idioma (ver [Iniciar el programa](#2-iniciar-el-programa)). |
| `remembered/`, `recent_projects.txt`, `last_folder.txt` en la carpeta de ajustes | Los ajustes guardados para la siguiente sesión, los proyectos recientes, la carpeta en la que empiezan los diálogos. Bórrelos para empezar de cero. |
| `docs/manual_es.md` | Este manual (español). |
| `docs/manual_en.md`, `docs/manual_cs.md`, `docs/manual_de.md`, `docs/manual_fr.md`, `docs/manual_pl.md` | El manual en inglés, checo, alemán, francés y polaco. |

Código fuente (para compilar el programa usted mismo):

| Ruta | Contenido |
|---|---|
| `Cargo.toml` | El proyecto y las bibliotecas que usa. |
| `src/image.rs`, `src/io/` | Imágenes; carga y guardado (JPEG, PNG, TIFF, BMP, RAW, FITS, XISF). |
| `src/stitcher/` | Unión: puntos clave, emparejado, colocación, refinado, proyecciones, fusión. |
| `src/stacker/` | Apilado de estrellas. |
| `src/finish.rs` | Posprocesado: geometría, estirados (MTF, arcsinh, GHS), balance de blancos, luz, curvas, color, detalle. |
| `src/gui/` | La ventana: las cuatro pestañas, los diálogos de archivos, el manual integrado y los textos de la ventana en seis idiomas (`i18n_*.rs`). |
| `src/bin/` | Los dos programas, `panostitch` y `panostack`. |
| `tests/` | Pruebas: apilado sobre escenas nocturnas sintéticas, unión, posprocesado, formatos de archivo y las pestañas de la ventana. Ejecútelas con `cargo test --release`. |
| `examples/scripted.rs` | Usar la unión desde su propio código Rust. |

---

## 16. Posprocesado

*Novedad en 0.4 (en desarrollo).* La pestaña **Posprocesado** hace los últimos
pasos sobre una imagen apilada o unida, para que no necesite otro programa:
girar y recortar, estirar datos lineales, balance de blancos, luz, curvas,
color, ruido, estrellas, enfoque y guardar al tamaño que quiera.
**Cargar una imagen:** **Abrir…** (Ctrl+O) o escriba una ruta y pulse Intro;
**✎ Usar en Posprocesado** en la pestaña Apilado de estrellas; o
**✎ Posprocesar** en la unión de panorámicas tras guardar una panorámica. Los
archivos FITS y XISF se tratan como datos lineales y empiezan con el estirado
automático; JPEG, TIFF y PNG se tratan como imágenes terminadas (sin estirado).
**Datos lineales (hay que estirar)** cambia esto a mano.

La vista previa trabaja sobre una copia de 2000 px como máximo, así sigue a los
deslizadores casi al instante. **⬇ Guardar** procesa la imagen completa en
segundo plano con exactamente los mismos ajustes; los valores que mide el
programa (estirado automático, fondo neutro) se toman de la vista previa, así
el archivo guardado se ve como la vista previa.

Los pasos siempre se ejecutan en este orden, de arriba abajo a la izquierda:

| Sección | Qué hace |
|---|---|
| **Girar y recortar** | ⟲ ⟳ cuartos de vuelta, **Espejo**, **Girar (°)** con cualquier ángulo (fino cerca de 0°, como guiñada/cabeceo/alabeo en la unión), **Quitar las esquinas vacías** tras girar, recorte por cada lado o recorte a una forma (3:2, 4:3, 16:9, 4:5, 1:1, 2:1). |
| **Estirado** (datos lineales) | Primero **Extracción de fondo** y **Calibración de color** (ambas opcionales, sobre los datos lineales), luego el método de estirado (abajo). |
| **Balance de blancos** | Sobre luz lineal, antes del estirado. **◎ Muestrear gris** y luego arrastre un rectángulo sobre algo que deba ser neutro (fondo del cielo, una nube): los multiplicadores de color se fijan a partir de ello. **Fondo neutro** vuelve gris la mitad más oscura. **Temperatura** y **Matiz** afinan. A las imágenes terminadas se les quita su curva sRGB para esto y se les devuelve, así el balance es correcto en todos los brillos. |
| **Luz** | **Exposición (EV)**, **Contraste**, **Luces** (en torno al 75 %), **Medios tonos** (50 %), **Sombras** (25 %), **Blancos** y **Negros** (los extremos), como en los editores de fotos. |
| **Curvas** | Una curva para todos los colores (**RGB**) y una para cada uno: **Rojo**, **Verde**, **Azul**. Clic para añadir un punto, arrastrar para moverlo, clic derecho para quitarlo. El histograma tras la curva es de lo que entra en ella. Un • marca las curvas que cambió. |
| **Color** | **Saturación** (0 = blanco y negro) e **Intensidad** (los colores apagados más que los fuertes). |
| **Detalle** | **Reducción de ruido** (suaviza zonas lisas, conserva bordes y estrellas), **Ruido de color**, **Reducción de estrellas** (estrellas más pequeñas), **Enfocar** y su radio. Los radios están en píxeles de la imagen completa, así la vista previa coincide con el resultado guardado. |
| **Cielo y primer plano** | Aspectos propios para el cielo y el primer plano, encima de todo lo anterior, mediante una máscara (ver abajo). |

**Métodos de estirado:**

| Método | Uso |
|---|---|
| **Desactivado** | La imagen ya está terminada. |
| **curva sRGB** | Como haría una cámara: para datos lineales que solo necesitan la gamma normal. |
| **MTF, automático** | Punto negro justo bajo el fondo del cielo, medios tonos para que el fondo quede con el brillo de **Fondo** (como el AutoStretch de Siril / el STF de PixInsight). **Cada color por separado** (por defecto) mide rojo, verde y azul por separado, para que la dominante de color de un apilado de cámara no oculte la imagen; los datos en sí no cambian. **Auto → manual** sigue a mano desde los valores medidos. |
| **MTF, manual** | Punto negro, medios tonos y punto blanco (función de transferencia de medios tonos). |
| **Arcsinh** | Seno hiperbólico inverso: estirado fuerte que conserva los colores de las estrellas. **Factor de estirado** 1–10000; los apilados profundos con fondo débil necesitan varios miles. |
| **Hiperbólico generalizado (GHS)** | **Estirado ln(D+1)** (fuerza), **Intensidad local b** (cuánto se concentra el estirado alrededor del punto de simetría), **Punto de simetría** (donde el estirado es más fuerte: póngalo en el fondo del cielo), **Proteger sombras** y **Proteger luces** (recta por debajo / por encima de ellos). |

Para Arcsinh y GHS: **Inversa** usa la curva inversa (comprime en vez de
estirar, p. ej. para deshacer un estirado demasiado fuerte); **Conservar
colores de estrellas** estira el brillo y conserva el color de cada píxel
(desactivado: cada color por separado, cielo más saturado, estrellas más
pálidas); **Puntos del fondo** pone el punto negro justo bajo el fondo del
color más oscuro y, en GHS, el punto de simetría sobre el fondo.

**Visualización** (sobre la vista previa) solo cambia cómo se muestra la vista
previa, nunca la imagen guardada, como los modos de visualización de Siril:
**Lineal (como se guarda)**, **Logaritmo**, **Raíz cuadrada**, **Al cuadrado**,
**Asinh**, **AutoStretch**, **Ecualización del histograma**. Útil para mirar en
las sombras o revisar el fondo sin cambiar nada. Mientras no sea Lineal se
muestra «solo vista».

**◐ Comparar** (manténgalo pulsado, o mantenga B) muestra el punto de partida:
girado, recortado y estirado, nada más.

**Guardar** (derecha): **Tamaño** (completo, o un lado mayor como 2048 px para
foros), nombre y tipo de archivo, **Calidad JPEG**. La línea de debajo dice
cuánta memoria necesita el guardado. FITS / XISF reciben la imagen terminada
(estirada) en coma flotante de 32 bits. **Mantener ajustes para la siguiente
imagen** (izquierda) da a varias imágenes el mismo aspecto, p. ej. los paneles
de una panorámica.

| Tecla | Acción |
|---|---|
| Ctrl+O | Abrir una imagen |
| Ctrl+S | Guardar (con el nombre de archivo mostrado) |
| Ctrl+Mayús+S | Guardar como… |
| F | Ajustar la vista previa |
| B (mantener) | Comparar con el punto de partida |
| Ctrl+Z, Ctrl+Y | Deshacer, rehacer |

**Cielo y primer plano** (cerrado al principio): un paisaje bajo las estrellas
suele necesitar tratar los dos de forma distinta, p. ej. menos ruido y
estrellas más pequeñas en el cielo, un primer plano más claro y cálido. Pinte
la máscara con **✎ Pintar** (botón izquierdo = cielo, derecho = primer plano,
rueda = tamaño del pincel) o deje que **◎ Detectar cielo** la encuentre (cielo
arriba; corríjala con el pincel; para un ojo de pez circular use **◯ Círculo
de ojo de pez**, capítulo 17), con **Todo cielo**, **Invertir** y **Vaciar**
para empezar de nuevo. Luego elija **Cielo** o **Primer plano** y ajuste su
luz, temperatura, color y detalle. **Borde suave** funde los dos aspectos en
el borde; **Mostrar máscara** tiñe el cielo de azul y el primer plano de
naranja (siempre visible al pintar). La máscara está sobre la propia imagen,
así se queda en su sitio al girar o recortar. El archivo guardado recibe los
mismos aspectos; un • tras el nombre de la sección indica que están en uso.

**↶ Deshacer** y **↷ Rehacer** (Ctrl+Z, Ctrl+Y o Ctrl+Mayús+Z) van hacia atrás
y hacia delante por los cambios; arrastrar un deslizador o un trazo de pincel
cuenta como un paso, y la ayuda emergente del botón dice qué deshace.

**✂ Recortar** recorta sobre la imagen: se muestra la imagen entera con un
marco; arrastre para dibujar un marco nuevo, arrastre sus bordes o esquinas
para ajustarlo, arrastre dentro para moverlo. Haga clic en **✂ Recorte:
hecho** para ver el resultado. Los botones de forma y los deslizadores de
recorte de **Girar y recortar** fijan el mismo marco.

**Preajustes** (arriba, bajo la imagen): escriba un nombre y **Guardar** para
conservar el aspecto actual: estirado, balance de blancos, luz, curvas, color,
detalle y los aspectos de cielo y primer plano, pero no el giro, el recorte
ni el tamaño de exportación (pertenecen a una imagen). **Preajuste** aplica un
aspecto guardado a la imagen abierta (Ctrl+Z lo deshace); **Borrar** quita el
preajuste cuyo nombre está escrito. Los preajustes se guardan en la carpeta de
ajustes (`presets`).

## 17. Timelapse

*Novedad en 0.4 (en desarrollo).* La pestaña **Timelapse** hace un vídeo a
partir de una secuencia de tomas: un paisaje bajo el cielo nocturno tomado
desde un trípode, con el cielo girando sobre un primer plano quieto. Un zoom
lento o un movimiento tipo slider a través de la toma pueden darle movimiento
de cámara.

**Proyectos:** **⬇ Guardar proyecto…** guarda las tomas, los aspectos, la
máscara (y el círculo de ojo de pez) y cada ajuste de vídeo en un archivo
`.tlproj`; **⇪ Abrir proyecto…** (Ctrl+O) lo recupera todo, también si la
carpeta se movió. Ctrl+S guarda directamente en el proyecto abierto. «Partir
del aspecto de la pestaña Posprocesado» se guarda como interruptor: usa la
imagen abierta en ese momento en Posprocesado.

**Tomas:** añada archivos o una carpeta; se reproducen en orden de nombre de
archivo (numeración de la cámara). Haga clic en una para mostrarla en el
centro; ◀ ▶ las recorren. Con tomas RAW, **RAW con** elige cómo se leen:
**integrado (rápido)** (por defecto) es rápido y trata todas las tomas igual;
**rawtherapee** aplica las ediciones guardadas en los archivos `.pp3` de
RawTherapee junto a las tomas, pero tarda varios segundos por toma (unas diez
veces más); **auto** usa RawTherapee solo para las tomas que tienen un archivo
`.pp3`. La pestaña dice cuántas tomas tienen uno. RawTherapee escribe un
`.pp3` para cada foto abierta en su navegador de archivos, así que tener uno
no significa que la foto se editara.

**Aspecto:** los mismos deslizadores de luz y color que en Posprocesado
(exposición, contraste, luces, medios tonos, sombras, blancos, negros,
temperatura, matiz, saturación, intensidad), ajustados por separado para
**Toda la toma**, el **Cielo** y el **Primer plano**. Cielo y primer plano van
encima del aspecto de toda la toma, solo dentro de su parte de la máscara; •
marca las partes que cambió. La toma seleccionada muestra el aspecto
(**Mostrar el aspecto**; desactivado muestra la toma tal cual). El deflicker y
el movimiento de cámara solo se ven en el vídeo.

**Máscara:** como en Posprocesado (capítulo 16): **✎ Pintar**, **◎ Detectar
cielo**, **Todo cielo**, **Invertir**, **Vaciar**, **Borde suave**, **Mostrar
máscara**. Una máscara sirve para todas las tomas, porque en trípode el primer
plano no se mueve.

Para un timelapse de todo el cielo con un ojo de pez circular apuntando
arriba, **◯ Círculo de ojo de pez** encuentra el círculo de imagen iluminado y
hace del cielo un disco en su centro; el anillo de fuera (horizonte, árboles,
luces) es primer plano. **Círculo del cielo** fija qué parte del radio del
círculo es cielo, y un **Borde suave** grande (hasta el 25 %) convierte el
borde en un degradado radial. Corrija el resto con el pincel, p. ej. un
telescopio o una persona que entre en el cielo.

**Vídeo:**

| Ajuste | Qué hace |
|---|---|
| **Deflicker** | Iguala los saltos de brillo y color entre tomas (pasos de exposición, luz cambiante, el balance de blancos de la cámara). **Suave** compara cada toma con sus vecinas (**Suavizado**, en tomas); **Fuerte** usa el triple. Los cambios lentos como el crepúsculo se mantienen. **También el color** iguala cada color por separado; **Por regiones** lo hace en una rejilla gruesa sobre la toma, así los cambios a gran escala en una parte (contaminación lumínica, una luz que pasa, el amanecer por un lado) se igualan donde ocurren. Ambos funcionan con objetivos normales y ojos de pez. La medición lee cada toma una vez más. |
| **Partir del aspecto de la pestaña Posprocesado** | Cada toma recibe primero los ajustes de la imagen abierta en Posprocesado (estirado, balance de blancos, luz, curvas, color, detalle; no su giro ni recorte), luego el aspecto ajustado aquí. Los valores medidos por el estirado automático vienen de la primera toma y sirven para todas, así el vídeo no parpadea. |
| **Movimiento de slider** | Como si la cámara fuera sobre un slider motorizado: el vídeo muestra una parte de la toma ese porcentaje más pequeña (hasta el 20 %) y la mueve por la toma, **Izquierda → derecha**, **Derecha → izquierda**, **Arriba** o **Abajo**. El movimiento es suave, no a saltos de píxel entero. |
| **Zoom lento** | Acerca lentamente durante todo el vídeo; se puede combinar con el movimiento de slider. |
| **Arrancar y parar suavemente** | El movimiento y el zoom aceleran al principio y frenan al final; desactivado: la misma velocidad todo el tiempo. La vista previa marca dónde empieza el vídeo (claro) y dónde termina (tenue). |

**Salida:**

| Ajuste | Qué hace |
|---|---|
| **Formato** | **MP4** (H.264) y **WebM** (VP9) se hacen con ffmpeg. Instálelo (ffmpeg.org; Windows: `winget install ffmpeg`; Linux: el gestor de paquetes; macOS: Homebrew) o póngalo junto al programa; la pestaña dice dónde lo encontró. **GIF animado** e **Imágenes numeradas** (TIFF o PNG de 16 bits, JPEG) no necesitan nada más. Los GIF de campos de estrellas son grandes (aprox. 1 MB por toma en 720p; MP4 es 5–10 veces más pequeño), así que van bien para clips cortos. |
| **Fotogramas por segundo** | 12–60 fotogramas por segundo. |
| **Resolución** | Como las tomas, 4K, 1440p, 1080p o 720p; nunca se amplía. Hasta 1080p, los RAW se leen a medio tamaño, lo que es más rápido y sigue siendo bastante nítido. |
| **Forma** | La forma del vídeo; la toma se recorta a ella y la vista previa marca el recorte. **Como las tomas** (sin recorte), **16:9**, **3:2** y **4:3** (siguen la orientación de las tomas), **1:1 cuadrado** (todo el círculo de un ojo de pez de todo el cielo) o **9:16 vertical** (móviles). En 1080p: 1920 × 1080, 1620 × 1080, 1440 × 1080, 1080 × 1080 y 1080 × 1920. Con un círculo de ojo de pez, la pestaña avisa cuando una forma corta parte del cielo. |
| **Calidad** | Para MP4, WebM e imágenes JPEG: más alta = mejor imagen, archivo más grande. |
| **Reproducir hacia delante y luego hacia atrás** | Las tomas otra vez al revés al final, así el vídeo hace un bucle suave. |
| **Mantener la última toma** | Mantiene la última toma unos segundos. |

Bajo los ajustes: cuántas tomas, cuánto dura el vídeo y su tamaño en píxeles.
**▶ Crear vídeo** trabaja en segundo plano, hasta ocho tomas a la vez (menos
cuando falta memoria), con una barra de progreso (tiempo transcurrido y
restante) y **✖ Cancelar** (no se guarda nada). Después un informe muestra las
tomas, el tamaño del vídeo, los fotogramas por segundo y la duración, el
tiempo empleado y el archivo con su tamaño, o qué salió mal. Por ejemplo, 69
tomas RAW (24 MP) se convirtieron en un MP4 de 1080p con deflicker en
aproximadamente medio minuto en un PC de sobremesa.
