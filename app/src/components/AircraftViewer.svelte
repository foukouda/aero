<script lang="ts">
  // The aircraft in 3D (three.js): orbit, zoom and pan, standard views, a
  // ground grid for scale, the centre of gravity and the mean chord.
  // Coordinates: aero-aircraft uses AVL's axes (x to the tail, y to the
  // right tip, z up); three.js has y up, so a point (x, y, z) is drawn at
  // (y, z, x) — a rotation, which keeps the triangles' orientation.
  import { onDestroy, onMount } from "svelte";
  import * as THREE from "three";
  import { OrbitControls } from "three/addons/controls/OrbitControls.js";
  import { CSS2DObject, CSS2DRenderer } from "three/addons/renderers/CSS2DRenderer.js";
  import type { Layout, MeshPart, PartKind, StripResult } from "../lib/bindings";
  import { app } from "../lib/state.svelte";
  import { length } from "../lib/mission";
  import { VIEW_DIRECTIONS, addLights, liftColors, partMesh, stallColor, type View } from "../lib/scene3d";

  interface Props {
    parts: MeshPart[];
    layout: Layout | null;
    /** Height of the wing root, m (where the CG marker is drawn). */
    wingZ: number;
    selected: PartKind | null;
    onselect: (kind: PartKind) => void;
    /** Strips of the 3D analysis: the surfaces are coloured by their lift. */
    strips?: StripResult[] | null;
    /** Neutral point of the 3D analysis, m behind the nose. */
    neutralPoint?: number | null;
  }

  let { parts, layout, wingZ, selected, onselect, strips = null, neutralPoint = null }: Props = $props();

  const VIEWS: View[] = ["threeQuarter", "top", "front", "side"];

  let host: HTMLDivElement | undefined = $state();
  let gridSize = $state(0);
  let renderer: THREE.WebGLRenderer | null = null;
  let labels: CSS2DRenderer | null = null;
  let camera: THREE.PerspectiveCamera | null = null;
  let controls: OrbitControls | null = null;
  const scene = new THREE.Scene();
  const aircraft = new THREE.Group();
  const markers = new THREE.Group();
  let grid: THREE.GridHelper | null = null;
  let observer: ResizeObserver | null = null;
  let framedRadius = 0;
  let view: View = "threeQuarter";

  scene.add(aircraft, markers);
  addLights(scene);

  function cssColor(name: string): string {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || "#2563eb";
  }

  function render() {
    if (!renderer || !camera) return;
    renderer.render(scene, camera);
    labels?.render(scene, camera);
  }

  function disposeGroup(group: THREE.Group) {
    for (const child of [...group.children]) {
      group.remove(child);
      child.traverse((o) => {
        const mesh = o as THREE.Mesh;
        mesh.geometry?.dispose();
        const material = mesh.material as THREE.Material | THREE.Material[] | undefined;
        if (Array.isArray(material)) material.forEach((m) => m.dispose());
        else material?.dispose();
        if (o instanceof CSS2DObject) o.element.remove();
      });
    }
  }

  /** The parts as three.js meshes, with their outline; the selected one
   * tinted with the accent colour. */
  function buildAircraft() {
    disposeGroup(aircraft);
    const accent = new THREE.Color(cssColor("--accent"));
    const [ok, warning, danger] = ["--ok", "--warning", "--danger"].map((v) => new THREE.Color(cssColor(v)));
    const color = (r: number | null) => stallColor(r, ok!, warning!, danger!);
    for (const part of parts) {
      const colors = strips ? liftColors(part, strips, color) : null;
      aircraft.add(partMesh(part, part.kind === selected ? accent : null, colors));
    }
  }

  function label(text: string): CSS2DObject {
    const el = document.createElement("span");
    el.className = "viewer-label";
    el.textContent = text;
    return new CSS2DObject(el);
  }

  /** Centre of gravity (the usual black and white quarters), mean chord,
   * ground grid. */
  function buildMarkers(radius: number, floor: number) {
    disposeGroup(markers);
    if (grid) {
      scene.remove(grid);
      grid.geometry.dispose();
      (grid.material as THREE.Material).dispose();
      grid = null;
    }
    if (!layout || !(radius > 0)) return;
    const r = Math.max(0.012 * layout.wing.span!, 0.004);
    const cg = new THREE.Group();
    for (let q = 0; q < 4; q++) {
      const quarter = new THREE.SphereGeometry(r, 16, 12, (q * Math.PI) / 2, Math.PI / 2);
      cg.add(
        new THREE.Mesh(quarter, new THREE.MeshBasicMaterial({ color: q % 2 ? 0x111111 : 0xffffff, side: THREE.DoubleSide })),
      );
    }
    cg.position.set(0, wingZ, layout.cg_x ?? 0);
    const cgLabel = label(app.t("viewer.cg"));
    cgLabel.position.set(0, r * 2.5, 0);
    cg.add(cgLabel);
    markers.add(cg);

    if (neutralPoint !== null && Number.isFinite(neutralPoint)) {
      // The neutral point: a diamond on the axis, its label below.
      const np = new THREE.Mesh(
        new THREE.OctahedronGeometry(r * 0.9),
        new THREE.MeshBasicMaterial({ color: new THREE.Color(cssColor("--accent")) }),
      );
      np.position.set(0, wingZ, neutralPoint);
      const npLabel = label(app.t("viewer.np"));
      npLabel.position.set(0, -r * 2.5, 0);
      np.add(npLabel);
      markers.add(np);
    }

    const w = layout.wing;
    if (w.mac_x !== null && w.mac !== null && w.mac_span !== null) {
      const y = w.mac_span;
      const geometry = new THREE.BufferGeometry().setFromPoints([
        new THREE.Vector3(y, wingZ + 0.6 * r, w.mac_x),
        new THREE.Vector3(y, wingZ + 0.6 * r, w.mac_x + w.mac),
      ]);
      const line = new THREE.Line(geometry, new THREE.LineBasicMaterial({ color: new THREE.Color(cssColor("--accent")) }));
      const macLabel = label(app.t("viewer.mac"));
      macLabel.position.set(y, wingZ + 3 * r, w.mac_x + 0.5 * w.mac);
      line.add(macLabel);
      markers.add(line);
    }

    // Grid squares of 1, 2 or 5 × 10^k m, about a tenth of the size.
    const target = (2 * radius) / 10;
    const power = 10 ** Math.floor(Math.log10(target));
    const step = [1, 2, 5, 10].map((m) => m * power).find((v) => v >= target) ?? target;
    const divisions = 16;
    gridSize = step;
    const gridColor = new THREE.Color(cssColor("--border"));
    grid = new THREE.GridHelper(step * divisions, divisions, gridColor, gridColor);
    const centre = new THREE.Box3().setFromObject(aircraft).getCenter(new THREE.Vector3());
    grid.position.set(centre.x, floor, centre.z);
    scene.add(grid);
  }

  function frame(force: boolean) {
    if (!camera || !controls) return;
    const box = new THREE.Box3().setFromObject(aircraft);
    if (box.isEmpty()) return;
    const sphere = box.getBoundingSphere(new THREE.Sphere());
    const floor = box.min.y - 0.15 * sphere.radius;
    buildMarkers(sphere.radius, floor);
    // Keep the user's view unless the aircraft's size changed a lot.
    if (!force && framedRadius > 0 && Math.abs(sphere.radius / framedRadius - 1) < 0.3) return;
    framedRadius = sphere.radius;
    setView(view, sphere);
  }

  function setView(v: View, sphere?: THREE.Sphere) {
    if (!camera || !controls) return;
    view = v;
    const s = sphere ?? new THREE.Box3().setFromObject(aircraft).getBoundingSphere(new THREE.Sphere());
    // The bounding sphere is generous for a flat aircraft: frame it tighter.
    const distance = (s.radius / Math.sin(THREE.MathUtils.degToRad(camera.fov / 2))) * 0.8;
    const dir = new THREE.Vector3(...VIEW_DIRECTIONS[v]).normalize();
    controls.target.copy(s.center);
    camera.position.copy(s.center).addScaledVector(dir, distance);
    camera.near = distance / 100;
    camera.far = distance * 100;
    camera.updateProjectionMatrix();
    controls.update();
    render();
  }

  function resize() {
    if (!host || !renderer || !camera) return;
    const { clientWidth: w, clientHeight: h } = host;
    if (w === 0 || h === 0) return;
    renderer.setSize(w, h);
    labels?.setSize(w, h);
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
    render();
  }

  // Click (without dragging) on a part selects it.
  let down: { x: number; y: number } | null = null;
  function pointerDown(e: PointerEvent) {
    down = { x: e.clientX, y: e.clientY };
  }
  function pointerUp(e: PointerEvent) {
    if (!down || !host || !camera) return;
    const moved = Math.hypot(e.clientX - down.x, e.clientY - down.y);
    down = null;
    if (moved > 4) return;
    const rect = host.getBoundingClientRect();
    const ndc = new THREE.Vector2(
      ((e.clientX - rect.left) / rect.width) * 2 - 1,
      -((e.clientY - rect.top) / rect.height) * 2 + 1,
    );
    const ray = new THREE.Raycaster();
    ray.setFromCamera(ndc, camera);
    const hit = ray.intersectObjects(aircraft.children, false)[0];
    const kind = hit?.object.userData.kind as PartKind | undefined;
    if (kind) onselect(kind);
  }

  onMount(() => {
    if (!host) return;
    renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true, preserveDrawingBuffer: true });
    renderer.setPixelRatio(window.devicePixelRatio);
    host.appendChild(renderer.domElement);
    labels = new CSS2DRenderer();
    labels.domElement.className = "viewer-labels";
    host.appendChild(labels.domElement);
    camera = new THREE.PerspectiveCamera(35, 1, 0.01, 1000);
    controls = new OrbitControls(camera, renderer.domElement);
    controls.addEventListener("change", render);
    renderer.domElement.addEventListener("pointerdown", pointerDown);
    renderer.domElement.addEventListener("pointerup", pointerUp);
    observer = new ResizeObserver(resize);
    observer.observe(host);
    resize();
    buildAircraft();
    frame(true);
  });

  onDestroy(() => {
    observer?.disconnect();
    controls?.dispose();
    disposeGroup(aircraft);
    disposeGroup(markers);
    grid?.geometry.dispose();
    renderer?.dispose();
    renderer?.domElement.remove();
    labels?.domElement.remove();
  });

  $effect(() => {
    // Rebuild when the parts, the selection, the layout or the theme change.
    void parts;
    void selected;
    void layout;
    void wingZ;
    void strips;
    void neutralPoint;
    void app.theme;
    if (!renderer) return;
    buildAircraft();
    frame(false);
    render();
  });
</script>

<div class="viewer">
  <div class="canvas" bind:this={host}></div>
  <div class="views" role="group" aria-label="views">
    {#each VIEWS as v (v)}
      <button class="small" onclick={() => setView(v)}>{app.t(`view.${v}`)}</button>
    {/each}
  </div>
  {#if strips}
    <ul class="lift-legend" aria-label={app.t("flies.colour")}>
      <li><span style:background="var(--ok)"></span>{app.t("flies.colour.far")}</li>
      <li><span style:background="var(--warning)"></span>{app.t("flies.colour.near")}</li>
      <li><span style:background="var(--danger)"></span>{app.t("flies.colour.stalled")}</li>
    </ul>
  {/if}
  <p class="hint muted">
    {app.t("viewer.hint")}
    {#if gridSize > 0}· {app.t("viewer.grid", { size: length(gridSize, app.tourLang) })}{/if}
  </p>
</div>

<style>
  .viewer {
    position: relative;
    height: 100%;
    min-height: 0;
  }
  .canvas {
    position: absolute;
    inset: 0;
    overflow: hidden;
    border-radius: var(--radius);
  }
  .canvas :global(canvas) {
    display: block;
    cursor: grab;
  }
  .canvas :global(.viewer-labels) {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }
  .canvas :global(.viewer-label) {
    font-size: 11px;
    font-weight: 600;
    color: var(--text);
    background: color-mix(in srgb, var(--panel) 80%, transparent);
    padding: 0 4px;
    border-radius: 3px;
  }
  .views {
    position: absolute;
    top: 8px;
    right: 8px;
    display: flex;
    gap: 4px;
  }
  .views button {
    background: color-mix(in srgb, var(--panel) 85%, transparent);
  }
  .small {
    font-size: 12px;
    padding: 2px 8px;
  }
  .lift-legend {
    position: absolute;
    right: 8px;
    bottom: 6px;
    margin: 0;
    padding: 2px 8px;
    list-style: none;
    display: flex;
    gap: 10px;
    font-size: 11px;
    border-radius: 4px;
    background: color-mix(in srgb, var(--panel) 85%, transparent);
    pointer-events: none;
  }
  .lift-legend li {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .lift-legend span {
    width: 10px;
    height: 10px;
    border-radius: 2px;
  }
  .hint {
    position: absolute;
    left: 10px;
    bottom: 6px;
    margin: 0;
    font-size: 11px;
    pointer-events: none;
  }
</style>
