import json, os, re, subprocess, sys

SRC = sys.argv[1]
OUT = sys.argv[2]
SIZE = 36

def read(p):
    return open(p, encoding="utf-8").read()

def strings(block):
    return re.findall(r"'((?:[^'\\]|\\.)*)'", block)

def parse_entries(text, keys):
    # Разбор TS-литералов вида { name: 'x', fileExtensions: [...], ... }
    entries = []
    for m in re.finditer(r"\{\s*name:\s*'([^']+)'", text):
        start = m.start()
        depth, i = 0, start
        while i < len(text):
            c = text[i]
            if c == "{":
                depth += 1
            elif c == "}":
                depth -= 1
                if depth == 0:
                    break
            i += 1
        body = text[start:i + 1]
        entry = {"name": m.group(1), "light": bool(re.search(r"light:\s*true", body))}
        for key in keys:
            km = re.search(key + r":\s*\[(.*?)\]", body, re.S)
            entry[key] = strings(km.group(1)) if km else []
        entries.append(entry)
    return entries

files_ts = read(f"{SRC}/src/core/icons/fileIcons.ts")
folders_ts = read(f"{SRC}/src/core/icons/folderIcons.ts")
file_entries = parse_entries(files_ts, ["fileExtensions", "fileNames"])
spec = folders_ts.split("name: 'specific'", 1)[1]
spec = spec.split("name: 'classic'", 1)[0]
folder_entries = parse_entries(spec, ["folderNames"])

icon_dir = f"{SRC}/icons"
have = {f[:-4] for f in os.listdir(icon_dir) if f.endswith(".svg")}
used = {"document", "folder-base", "folder-base-open", "folder-root", "folder-root-open"}
OPEN_D = "M14.483 6H4.721a1 1 0 0 0-.949.684L2 12V5h12a1 1 0 0 0-1-1H7.562a1 1 0 0 1-.64-.232l-.644-.536A1 1 0 0 0 5.638 3H2a1 1 0 0 0-1 1v8a1 1 0 0 0 1 1h11l2.403-5.606A1 1 0 0 0 14.483 6"
ROOT = "#90a4ae"
root_svgs = {
    "folder-root": f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><circle cx="8" cy="8" r="6" fill="none" stroke="{ROOT}" stroke-width="2"/><circle cx="8" cy="8" r="3" fill="{ROOT}"/></svg>',
    "folder-root-open": f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><circle cx="8" cy="8" r="6" fill="none" stroke="{ROOT}" stroke-width="2"/></svg>',
}

def make_open(name):
    text = open(f"{SRC}/icons/{name}.svg", encoding="utf-8").read()
    return re.sub(r'(<path id="folder"[^>]*?\sd=")[^"]*(")', lambda m: m.group(1) + OPEN_D + m.group(2), text, count=1)

lines = []
light = set()
for e in file_entries:
    if e["name"] not in have:
        continue
    used.add(e["name"])
    if e["light"] and e["name"] + "_light" in have:
        light.add(e["name"])
        used.add(e["name"] + "_light")
    for x in e["fileExtensions"]:
        lines.append(f"ext {x.lower()} {e['name']}")
    for n in e["fileNames"]:
        lines.append(f"file {n.lower()} {e['name']}")
for e in folder_entries:
    if e["name"] not in have:
        continue
    used.add(e["name"])
    used.add(e["name"] + "-open")
    for n in e["folderNames"]:
        lines.append(f"dir {n.lower()} {e['name']}")

os.makedirs(OUT, exist_ok=True)
# Свои значки IC лежат рядом с генератором
custom = {"ic-scene": "tools/icons/ic-scene.svg", "folder-open": None}
pak = bytearray()
index = []
tmp = "/tmp/_icon.png"
for name in sorted(used):
    svg = f"{icon_dir}/{name}.svg"
    if name in root_svgs:
        svg = "/tmp/_root.svg"
        open(svg, "w").write(root_svgs[name])
    elif name.endswith("-open") and name[:-5] + "" in have:
        svg = "/tmp/_open.svg"
        open(svg, "w", encoding="utf-8").write(make_open(name[:-5]))
    elif name == "folder-base-open":
        svg = "/tmp/_open.svg"
        open(svg, "w", encoding="utf-8").write(make_open("folder-base"))
    elif not os.path.exists(svg):
        continue
    subprocess.run(["rsvg-convert", "-w", str(SIZE), "-h", str(SIZE), svg, "-o", tmp], check=True)
    data = open(tmp, "rb").read()
    index.append((name, len(pak), len(data)))
    pak += data
for name, svg in [("ic-scene", "tools/icons/ic-scene.svg")]:
    subprocess.run(["rsvg-convert", "-w", str(SIZE), "-h", str(SIZE), svg, "-o", tmp], check=True)
    data = open(tmp, "rb").read()
    index.append((name, len(pak), len(data)))
    pak += data

open(f"{OUT}/icons.pak", "wb").write(pak)
out = [f"png {n} {o} {l}" for n, o, l in index]
out += [f"light {n}" for n in sorted(light)]
out += lines
out += ["ext ic-scene ic-scene", "ext lang i18n", "ext theme palette"]
open(f"{OUT}/icon_map.txt", "w").write("\n".join(out) + "\n")
print(len(index), "icons,", len(lines), "mappings,", len(pak) // 1024, "KB")
