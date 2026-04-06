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
  let animId: number;
  let mesh: THREE.Mesh;
  let isSpinning = $state(spin);

  // Keep reactive to spin prop
  $effect(() => { isSpinning = spin; });

  onMount(() => {
    const W = size, H = size;
    const DEPTH = Math.max(3, size * 0.022); // proportional thickness

    renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    renderer.setPixelRatio(window.devicePixelRatio);
    renderer.setSize(W, H);
    renderer.setClearColor(0x000000, 0);
    container.appendChild(renderer.domElement);

    const scene = new THREE.Scene();
    const camera = new THREE.PerspectiveCamera(28, W / H, 0.1, 1000);
    camera.position.z = size * 2.15;

    const geo = new THREE.BoxGeometry(size * 0.77, size * 0.77, DEPTH);

    const sideMat = new THREE.MeshStandardMaterial({ color: 0x1a1a2e, roughness: 0.6 });
    const materials: THREE.Material[] = [
      sideMat, sideMat, sideMat, sideMat, sideMat, sideMat,
    ];

    mesh = new THREE.Mesh(geo, materials);
    scene.add(mesh);

    if (src) {
      // ImageBitmapLoader decodes off the main thread — no freeze for large images
      const loader = new THREE.ImageBitmapLoader();
      loader.setOptions({ imageOrientation: 'flipY' });
      loader.load(src, (bitmap) => {
        const tex = new THREE.Texture(bitmap);
        tex.colorSpace = THREE.SRGBColorSpace;
        tex.needsUpdate = true;
        const frontMat = new THREE.MeshStandardMaterial({ map: tex });
        const texBack = tex.clone();
        texBack.repeat.set(-1, 1);
        texBack.offset.set(1, 0);
        texBack.needsUpdate = true;
        const backMat = new THREE.MeshStandardMaterial({ map: texBack });
        materials[4] = frontMat;
        materials[5] = backMat;
        mesh.material = [...materials];
      });
    }

    const ambient = new THREE.AmbientLight(0xffffff, 0.55);
    scene.add(ambient);
    const key = new THREE.DirectionalLight(0xffffff, 1.1);
    key.position.set(3, 4, 6);
    scene.add(key);
    const fill = new THREE.DirectionalLight(0xaaaacc, 0.35);
    fill.position.set(-4, -2, 3);
    scene.add(fill);

    const RPM = 12;
    let lastTime = performance.now();

    function animate() {
      animId = requestAnimationFrame(animate);
      const now = performance.now();
      const dt = (now - lastTime) / 1000;
      lastTime = now;
      if (isSpinning) {
        mesh.rotation.y += dt * RPM * Math.PI * 2 / 60;
      }
      renderer.render(scene, camera);
    }
    animate();
  });

  onDestroy(() => {
    cancelAnimationFrame(animId);
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
    filter: drop-shadow(0 8px 24px rgba(0, 0, 0, 0.45));
  }
</style>
