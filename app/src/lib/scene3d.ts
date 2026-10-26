// three.js pieces shared by the aircraft view and the gallery thumbnails.
// Coordinates: aero-aircraft uses AVL's axes (x to the tail, y to the right
// tip, z up); three.js has y up, so a point (x, y, z) is drawn at (y, z, x)
// — a rotation, which keeps the triangles' orientation.

import * as THREE from "three";
import type { MeshPart, PartKind } from "./bindings";

export const PART_COLORS: Record<PartKind, number> = {
  wing: 0xc9d4e2,
  canard: 0xc9d4e2,
  tail: 0xb6c3d3,
  fin: 0xb6c3d3,
  fuselage: 0x98a6b8,
};

export type View = "threeQuarter" | "top" | "front" | "side";

/** Directions from the aircraft to the camera (the nose points to −z, the
 * right wing to +x). */
export const VIEW_DIRECTIONS: Record<View, [number, number, number]> = {
  threeQuarter: [-1.1, 0.75, -1.3],
  top: [0, 1, 0.0001],
  front: [0, 0.12, -1],
  side: [-1, 0.08, 0],
};

/** A part as a three.js mesh with its outline; `highlight` tints it. */
export function partMesh(part: MeshPart, highlight: THREE.Color | null = null): THREE.Mesh {
  const p = part.positions;
  const positions = new Float32Array(p.length);
  for (let i = 0; i < p.length; i += 3) {
    positions[i] = p[i + 1] ?? 0;
    positions[i + 1] = p[i + 2] ?? 0;
    positions[i + 2] = p[i] ?? 0;
  }
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
  geometry.setIndex(part.indices.map((i) => i ?? 0));
  geometry.computeVertexNormals();
  const material = new THREE.MeshStandardMaterial({
    color: PART_COLORS[part.kind],
    roughness: 0.6,
    metalness: 0.05,
    side: THREE.DoubleSide,
    emissive: highlight ?? new THREE.Color(0x000000),
    emissiveIntensity: highlight ? 0.35 : 0,
  });
  const mesh = new THREE.Mesh(geometry, material);
  mesh.userData.kind = part.kind;
  mesh.add(
    new THREE.LineSegments(
      new THREE.EdgesGeometry(geometry, 35),
      new THREE.LineBasicMaterial({ color: highlight ?? 0x5b6676, transparent: true, opacity: 0.55 }),
    ),
  );
  return mesh;
}

/** Lights of every aircraft scene. */
export function addLights(scene: THREE.Scene) {
  scene.add(new THREE.HemisphereLight(0xffffff, 0x8899aa, 1.6));
  const sun = new THREE.DirectionalLight(0xffffff, 1.8);
  sun.position.set(2, 4, -3);
  scene.add(sun);
}

/** Frees the geometries and materials of `object`'s children. */
export function disposeChildren(object: THREE.Object3D) {
  for (const child of [...object.children]) {
    object.remove(child);
    child.traverse((o) => {
      const mesh = o as THREE.Mesh;
      mesh.geometry?.dispose();
      const material = mesh.material as THREE.Material | THREE.Material[] | undefined;
      if (Array.isArray(material)) material.forEach((m) => m.dispose());
      else material?.dispose();
    });
  }
}

/** Places `camera` to see `object` from `view`; `tightness` below 1 frames
 * closer than the bounding sphere. Returns the sphere. */
export function frame(
  camera: THREE.PerspectiveCamera,
  object: THREE.Object3D,
  view: View,
  tightness = 0.8,
): THREE.Sphere {
  const sphere = new THREE.Box3().setFromObject(object).getBoundingSphere(new THREE.Sphere());
  const distance = (sphere.radius / Math.sin(THREE.MathUtils.degToRad(camera.fov / 2))) * tightness;
  place(camera, sphere.center, view, distance);
  return sphere;
}

/** Places `camera` on the `view` direction, `distance` from `target`. */
function place(camera: THREE.PerspectiveCamera, target: THREE.Vector3, view: View, distance: number) {
  const dir = new THREE.Vector3(...VIEW_DIRECTIONS[view]).normalize();
  camera.position.copy(target).addScaledVector(dir, distance);
  camera.near = distance / 100;
  camera.far = distance * 100;
  camera.lookAt(target);
  camera.updateProjectionMatrix();
  camera.updateMatrixWorld();
}

/** Frames `object` so that its bounding box fills `fill` of the picture in
 * its tighter direction, whatever the picture's proportions. */
export function frameToFit(camera: THREE.PerspectiveCamera, object: THREE.Object3D, view: View, fill = 0.9) {
  const box = new THREE.Box3().setFromObject(object);
  const centre = box.getCenter(new THREE.Vector3());
  const corners = [0, 1, 2, 3, 4, 5, 6, 7].map(
    (i) =>
      new THREE.Vector3(
        i & 1 ? box.max.x : box.min.x,
        i & 2 ? box.max.y : box.min.y,
        i & 4 ? box.max.z : box.min.z,
      ),
  );
  let distance = (box.getSize(new THREE.Vector3()).length() / 2 / Math.sin(THREE.MathUtils.degToRad(camera.fov / 2))) * 0.8;
  for (let k = 0; k < 4; k++) {
    place(camera, centre, view, distance);
    const extent = Math.max(
      ...corners.map((c) => {
        const p = c.clone().project(camera);
        return Math.max(Math.abs(p.x), Math.abs(p.y));
      }),
    );
    if (!(extent > 0)) break;
    // The projection scales roughly as 1 / distance.
    distance *= extent / fill;
  }
  place(camera, centre, view, distance);
}

/** Pictures of aircraft (PNG data URLs) from the same angle, rendered one
 * after the other with a single WebGL context. */
export function renderThumbnails(aircraft: MeshPart[][], width: number, height: number): string[] {
  const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true, preserveDrawingBuffer: true });
  const ratio = Math.min(window.devicePixelRatio || 1, 2);
  renderer.setPixelRatio(ratio);
  renderer.setSize(width, height, false);
  const scene = new THREE.Scene();
  addLights(scene);
  const group = new THREE.Group();
  scene.add(group);
  const camera = new THREE.PerspectiveCamera(30, width / height, 0.01, 1000);
  const out: string[] = [];
  try {
    for (const parts of aircraft) {
      disposeChildren(group);
      for (const part of parts) group.add(partMesh(part));
      frameToFit(camera, group, "threeQuarter", 0.92);
      renderer.render(scene, camera);
      out.push(renderer.domElement.toDataURL("image/png"));
    }
  } finally {
    disposeChildren(group);
    renderer.dispose();
    renderer.forceContextLoss();
  }
  return out;
}
