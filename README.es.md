# Recents Filter

Mantén carpetas seleccionadas fuera de la lista de archivos recientes de GNOME, en todas las aplicaciones.

![Licencia Pública General Affero de GNU v3](https://img.shields.io/badge/Licencia-AGPL--3.0-blue.svg)

## Por qué existe

GNOME no tiene exclusión por carpeta para Archivos recientes. Las aplicaciones GTK registran todo lo que abres en `~/.local/share/recently-used.xbel`, y no hay ningún ajuste que diga "esta carpeta nunca aparece aquí". El truco habitual — ocultar una carpeta poniéndole un punto delante del nombre — solo esconde sus entradas en Archivos y en los diálogos de GTK. Las entradas siguen quedando en el xbel, las aplicaciones de terceros las muestran, y activar *Mostrar archivos ocultos* lo expone todo otra vez.

Recents Filter cierra ese hueco con dos piezas pequeñas:

- **Una GUI (GTK4/libadwaita)** para gestionar la lista de carpetas excluidas. Vive en `~/.config/recents-filter/config.json`.
- **Una purga al escribir, disparada por systemd.** Una unit `recents-filterd.path` (el vigilante de ficheros por inotify de systemd) arranca un one-shot `recents-filterd.service` en cuanto cambia `recently-used.xbel`. El one-shot elimina todo bookmark cuya ruta esté bajo una carpeta excluida y vuelve a escribir el fichero atómicamente. **No hay daemon residente**: entre escrituras solo espera systemd, sin procesos propios corriendo.

## Instalación

```bash
cargo build --release
cp target/release/recents-filter target/release/recents-filterd ~/.local/bin/
mkdir -p ~/.config/systemd/user ~/.local/share/applications ~/.local/share/metainfo
cp data/recents-filterd.path data/recents-filterd.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now recents-filterd.path
cp data/org.gnacho.RecentsFilter.desktop ~/.local/share/applications/
cp data/org.gnacho.RecentsFilter.metainfo.xml ~/.local/share/metainfo/
update-desktop-database ~/.local/share/applications
```

Después lanza **Recents Filter** desde tu cuadrícula de aplicaciones y añade las carpetas que quieras mantener fuera de Archivos recientes. La GUI muestra el estado del watcher y ofrece *Enable watcher* (si la unit `.path` está apagada) y *Purge now* (ejecutar el one-shot a mano).

## Cómo se comporta

- **Disparado, no por sondeo**: systemd vigila `recently-used.xbel` vía inotify y arranca el one-shot solo cuando cambia. No corre ningún proceso entre escrituras.
- **Instantáneo**: un fichero abierto en una carpeta excluida desaparece de Archivos recientes en menos de un segundo.
- **Atómico**: el xbel se reescribe con fichero temporal + rename, así que una aplicación GTK que lo lea a la vez nunca ve un fichero a medias.
- **Privado**: el xbel reescrito conserva el modo `0600` que GTK usa para ese fichero.
- **Converge**: cuando una aplicación GTK añade un reciente no excluido, reescribe todo el xbel desde su lista en memoria — incluidas las entradas excluidas. El one-shot las purga otra vez. Se estabiliza en segundos, sin bucle.

## Desarrollo

```bash
cargo test    # tests unitarios del parser/purger del xbel
cargo build   # build de debug de los dos binarios
```

## Licencia

[AGPL-3.0](LICENSE)
