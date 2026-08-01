<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import * as THREE from 'three';

  let {
    src,
    alt = '',
    size = 260,
    spin = true,
  }: {
    src: string | null;
    alt?: string;
    size?: number;
    spin?: boolean;
  } = $props();

  let container: HTMLDivElement;
  let renderer: THREE.WebGLRenderer;
  let animId: number = 0;
  let mesh: THREE.Mesh;
  let scene: THREE.Scene;
  let camera: THREE.PerspectiveCamera;
  let isSpinning = $derived(spin);

  function renderSingleFrame() {
    if (renderer && scene && camera) {
      renderer.render(scene, camera);
    }
  }

  function loadTexture(texSrc: string) {
    if (!mesh) return;
    const mats = mesh.material as THREE.Material[];
    const sideMat = new THREE.MeshStandardMaterial({ color: 0x1a1a2e, roughness: 0.6 });
    const loader = new THREE.ImageBitmapLoader();
    loader.setOptions({ imageOrientation: 'flipY' });
    loader.load(texSrc, (bitmap) => {
      const tex = new THREE.Texture(bitmap);
      tex.colorSpace = THREE.SRGBColorSpace;
      tex.needsUpdate = true;
      const frontMat = new THREE.MeshStandardMaterial({ map: tex });
      const texBack = tex.clone();
      texBack.repeat.set(-1, 1);
      texBack.offset.set(1, 0);
      texBack.needsUpdate = true;
      const backMat = new THREE.MeshStandardMaterial({ map: texBack });
      // dispose old face materials before replacing
      if (mats[4] !== sideMat) (mats[4] as THREE.MeshStandardMaterial).map?.dispose(), mats[4].dispose();
      if (mats[5] !== sideMat) (mats[5] as THREE.MeshStandardMaterial).map?.dispose(), mats[5].dispose();
      mats[4] = frontMat;
      mats[5] = backMat;
      mesh.material = [...mats];
      renderSingleFrame();
    });
  }

  let lastTime = 0;
  const RPM = 12;

  function animate() {
    if (!isSpinning) {
      renderSingleFrame();
      animId = 0;
      return;
    }
    animId = requestAnimationFrame(animate);
    const now = performance.now();
    const dt = lastTime ? (now - lastTime) / 1000 : 0;
    lastTime = now;
    if (mesh) {
      mesh.rotation.y += dt * RPM * Math.PI * 2 / 60;
    }
    renderSingleFrame();
  }

  // React to src changes after mount
  $effect(() => {
    if (src && mesh) loadTexture(src);
  });

  // React to spin state changes
  $effect(() => {
    if (isSpinning) {
      if (!animId && renderer) {
        lastTime = performance.now();
        animate();
      }
    } else {
      if (animId) {
        cancelAnimationFrame(animId);
        animId = 0;
        renderSingleFrame();
      }
    }
  });

  onMount(() => {
    const W = size, H = size;
    const DEPTH = Math.max(3, size * 0.022); // proportional thickness

    renderer = new THREE.WebGLRenderer({
      antialias: true,
      alpha: true,
      powerPreference: 'high-performance',
    });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.setSize(W, H);
    renderer.setClearColor(0x000000, 0);
    container.appendChild(renderer.domElement);

    scene = new THREE.Scene();
    camera = new THREE.PerspectiveCamera(28, W / H, 0.1, 1000);
    camera.position.z = size * 2.15;

    const geo = new THREE.BoxGeometry(size * 0.77, size * 0.77, DEPTH);

    const sideMat = new THREE.MeshStandardMaterial({ color: 0x1a1a2e, roughness: 0.6 });
    const materials: THREE.Material[] = [
      sideMat, sideMat, sideMat, sideMat, sideMat, sideMat,
    ];

    mesh = new THREE.Mesh(geo, materials);
    scene.add(mesh);

    if (src) loadTexture(src);

    const ambient = new THREE.AmbientLight(0xffffff, 0.55);
    scene.add(ambient);
    const key = new THREE.DirectionalLight(0xffffff, 1.1);
    key.position.set(3, 4, 6);
    scene.add(key);
    const fill = new THREE.DirectionalLight(0xaaaacc, 0.35);
    fill.position.set(-4, -2, 3);
    scene.add(fill);

    renderSingleFrame();
    if (isSpinning) {
      lastTime = performance.now();
      animate();
    }
  });

  onDestroy(() => {
    if (animId) cancelAnimationFrame(animId);
    if (mesh) {
      mesh.geometry?.dispose();
      if (Array.isArray(mesh.material)) {
        mesh.material.forEach((m) => {
          if (m instanceof THREE.MeshStandardMaterial) {
            m.map?.dispose();
          }
          m.dispose();
        });
      }
    }
    renderer?.dispose();
  });
</script>

<div class="cover-wrap" style="width:{size}px;height:{size}px" bind:this={container}></div>

<style>
  .cover-wrap {
    flex-shrink: 0;
  }

  .cover-wrap :global(canvas) {
    display: block;
    filter: drop-shadow(0 8px 16px rgba(0, 0, 0, 0.4));
  }
</style>
