(() => {
  const canvas = document.getElementById("bgCanvas");
  if (!canvas) return;

  const renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  renderer.setSize(window.innerWidth, window.innerHeight);

  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(60, window.innerWidth / window.innerHeight, 0.1, 1000);
  camera.position.z = 1;

  const starCount = 1800;
  const starGeo = new THREE.BufferGeometry();
  const positions = new Float32Array(starCount * 3);
  const colors = new Float32Array(starCount * 3);
  const sizes = new Float32Array(starCount);

  for (let i = 0; i < starCount; i++) {
    const i3 = i * 3;
    positions[i3] = (Math.random() - 0.5) * 10;
    positions[i3 + 1] = (Math.random() - 0.5) * 10;
    positions[i3 + 2] = (Math.random() - 0.5) * 5;

    const warmth = Math.random();
    if (warmth < 0.15) {
      colors[i3] = 0.6; colors[i3 + 1] = 0.75; colors[i3 + 2] = 1.0;
    } else if (warmth < 0.3) {
      colors[i3] = 1.0; colors[i3 + 1] = 0.85; colors[i3 + 2] = 0.6;
    } else {
      colors[i3] = 0.95; colors[i3 + 1] = 0.93; colors[i3 + 2] = 0.88;
    }

    sizes[i] = Math.random() * 2.5 + 0.5;
  }

  starGeo.setAttribute("position", new THREE.BufferAttribute(positions, 3));
  starGeo.setAttribute("color", new THREE.BufferAttribute(colors, 3));
  starGeo.setAttribute("size", new THREE.BufferAttribute(sizes, 1));

  const starMat = new THREE.ShaderMaterial({
    uniforms: { uTime: { value: 0 } },
    vertexShader: `
      attribute float size;
      attribute vec3 color;
      varying vec3 vColor;
      varying float vAlpha;
      uniform float uTime;
      void main() {
        vColor = color;
        vec3 pos = position;
        pos.x += sin(uTime * 0.15 + position.y * 2.0) * 0.015;
        pos.y += cos(uTime * 0.12 + position.x * 2.0) * 0.015;
        vec4 mv = modelViewMatrix * vec4(pos, 1.0);
        gl_PointSize = size * (200.0 / -mv.z);
        gl_Position = projectionMatrix * mv;
        vAlpha = 0.6 + 0.4 * sin(uTime * 0.5 + position.x * 3.0 + position.y * 2.0);
      }
    `,
    fragmentShader: `
      varying vec3 vColor;
      varying float vAlpha;
      void main() {
        float d = length(gl_PointCoord - vec2(0.5));
        if (d > 0.5) discard;
        float glow = 1.0 - smoothstep(0.0, 0.5, d);
        gl_FragColor = vec4(vColor, glow * glow * vAlpha);
      }
    `,
    transparent: true,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
  });

  const stars = new THREE.Points(starGeo, starMat);
  scene.add(stars);

  const nebulaCount = 600;
  const nebulaGeo = new THREE.BufferGeometry();
  const nPos = new Float32Array(nebulaCount * 3);
  const nCol = new Float32Array(nebulaCount * 3);
  const nSizes = new Float32Array(nebulaCount);

  for (let i = 0; i < nebulaCount; i++) {
    const i3 = i * 3;
    const angle = Math.random() * Math.PI * 2;
    const radius = Math.random() * 3.5 + 0.5;
    nPos[i3] = Math.cos(angle) * radius;
    nPos[i3 + 1] = Math.sin(angle) * radius * 0.6;
    nPos[i3 + 2] = (Math.random() - 0.5) * 2 - 1;

    const hue = Math.random();
    if (hue < 0.33) {
      nCol[i3] = 0.15; nCol[i3 + 1] = 0.1; nCol[i3 + 2] = 0.35;
    } else if (hue < 0.66) {
      nCol[i3] = 0.08; nCol[i3 + 1] = 0.15; nCol[i3 + 2] = 0.3;
    } else {
      nCol[i3] = 0.2; nCol[i3 + 1] = 0.06; nCol[i3 + 2] = 0.18;
    }

    nSizes[i] = Math.random() * 40 + 15;
  }

  nebulaGeo.setAttribute("position", new THREE.BufferAttribute(nPos, 3));
  nebulaGeo.setAttribute("color", new THREE.BufferAttribute(nCol, 3));
  nebulaGeo.setAttribute("size", new THREE.BufferAttribute(nSizes, 1));

  const nebulaMat = new THREE.ShaderMaterial({
    uniforms: { uTime: { value: 0 } },
    vertexShader: `
      attribute float size;
      attribute vec3 color;
      varying vec3 vColor;
      varying float vAlpha;
      uniform float uTime;
      void main() {
        vColor = color;
        vec3 pos = position;
        pos.x += sin(uTime * 0.03 + position.y) * 0.08;
        pos.y += cos(uTime * 0.025 + position.x) * 0.06;
        vec4 mv = modelViewMatrix * vec4(pos, 1.0);
        gl_PointSize = size * (300.0 / -mv.z);
        gl_Position = projectionMatrix * mv;
        vAlpha = 0.08 + 0.04 * sin(uTime * 0.2 + position.x);
      }
    `,
    fragmentShader: `
      varying vec3 vColor;
      varying float vAlpha;
      void main() {
        float d = length(gl_PointCoord - vec2(0.5));
        if (d > 0.5) discard;
        float soft = 1.0 - smoothstep(0.0, 0.5, d);
        float glow = soft * soft * soft;
        gl_FragColor = vec4(vColor, glow * vAlpha);
      }
    `,
    transparent: true,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
  });

  const nebula = new THREE.Points(nebulaGeo, nebulaMat);
  scene.add(nebula);

  let mouseX = 0;
  let mouseY = 0;
  document.addEventListener("mousemove", (e) => {
    mouseX = (e.clientX / window.innerWidth - 0.5) * 2;
    mouseY = (e.clientY / window.innerHeight - 0.5) * 2;
  });

  function onResize() {
    camera.aspect = window.innerWidth / window.innerHeight;
    camera.updateProjectionMatrix();
    renderer.setSize(window.innerWidth, window.innerHeight);
  }
  window.addEventListener("resize", onResize);

  const clock = new THREE.Clock();

  function animate() {
    requestAnimationFrame(animate);
    const t = clock.getElapsedTime();

    starMat.uniforms.uTime.value = t;
    nebulaMat.uniforms.uTime.value = t;

    stars.rotation.y = t * 0.012 + mouseX * 0.08;
    stars.rotation.x = t * 0.008 + mouseY * 0.05;

    nebula.rotation.y = t * 0.006 + mouseX * 0.03;
    nebula.rotation.z = t * 0.004;

    renderer.render(scene, camera);
  }

  animate();
})();
