/**
 * Uploads raw RGBA frame buffers (as delivered by the backend's ffmpeg
 * frame-pipe over a Tauri Channel) to a WebGL2 texture and draws them onto a
 * fullscreen quad. Plain class (not a Svelte component) so it can be shared
 * between the real player and the stage-1 debug harness without coupling to
 * component lifecycle.
 */
export class FrameRenderer {
  private canvas: HTMLCanvasElement;
  private gl: WebGL2RenderingContext;
  private program: WebGLProgram;
  private vao: WebGLVertexArrayObject;
  private vbo: WebGLBuffer;
  private texture: WebGLTexture;
  private texUniformLoc: WebGLUniformLocation | null;
  private width: number;
  private height: number;

  constructor(canvas: HTMLCanvasElement, width: number, height: number) {
    const gl = canvas.getContext('webgl2', {
      antialias: false,
      alpha: false,
      depth: false,
      stencil: false,
      powerPreference: 'high-performance',
    });
    if (!gl) throw new Error('WebGL2 is not supported on this platform');

    this.canvas = canvas;
    this.gl = gl;
    this.width = width;
    this.height = height;
    canvas.width = width;
    canvas.height = height;

    const vs = compileShader(gl, gl.VERTEX_SHADER, VERTEX_SRC);
    const fs = compileShader(gl, gl.FRAGMENT_SHADER, FRAGMENT_SRC);
    this.program = linkProgram(gl, vs, fs);
    gl.deleteShader(vs);
    gl.deleteShader(fs);
    this.texUniformLoc = gl.getUniformLocation(this.program, 'uTex');

    const vao = gl.createVertexArray();
    if (!vao) throw new Error('Failed to create VAO');
    this.vao = vao;
    const vbo = gl.createBuffer();
    if (!vbo) throw new Error('Failed to create VBO');
    this.vbo = vbo;

    gl.bindVertexArray(vao);
    gl.bindBuffer(gl.ARRAY_BUFFER, vbo);
    // Fullscreen quad as a triangle strip, clip-space positions.
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
    gl.bindVertexArray(null);

    this.texture = createTexture(gl, width, height);
    gl.viewport(0, 0, width, height);
  }

  /** Uploads a frame's raw RGBA bytes into the texture. Does not draw. */
  uploadFrame(data: ArrayBuffer): void {
    const gl = this.gl;
    const expected = this.width * this.height * 4;
    if (data.byteLength !== expected) {
      console.warn(`FrameRenderer: frame size mismatch (got ${data.byteLength}, expected ${expected})`);
      return;
    }
    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, this.width, this.height, gl.RGBA, gl.UNSIGNED_BYTE, new Uint8Array(data));
    gl.bindTexture(gl.TEXTURE_2D, null);
  }

  /** Renders whatever is currently in the texture. Cheap — safe to call every rAF tick. */
  draw(): void {
    const gl = this.gl;
    gl.useProgram(this.program);
    gl.bindVertexArray(this.vao);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    if (this.texUniformLoc) gl.uniform1i(this.texUniformLoc, 0);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
    gl.bindVertexArray(null);
  }

  /** Recreates the texture at a new fixed frame size (e.g. switching to a differently-sized clip). */
  resize(width: number, height: number): void {
    const gl = this.gl;
    this.width = width;
    this.height = height;
    this.canvas.width = width;
    this.canvas.height = height;
    gl.deleteTexture(this.texture);
    this.texture = createTexture(gl, width, height);
    gl.viewport(0, 0, width, height);
  }

  dispose(): void {
    const gl = this.gl;
    gl.deleteTexture(this.texture);
    gl.deleteBuffer(this.vbo);
    gl.deleteVertexArray(this.vao);
    gl.deleteProgram(this.program);
    // Deleting individual GL objects does not release the context itself —
    // that only happens on GC, which is not prompt enough when a new
    // context is created every time a clip opens (ClipPlayerView mounts a
    // fresh <canvas> per clip). WEBGL_lose_context.loseContext() is the
    // documented way to force immediate release; without it, contexts pile
    // up across repeated open/close and the driver crashes once system
    // limits are hit (observed as a SIGSEGV inside the NVIDIA GL driver).
    gl.getExtension('WEBGL_lose_context')?.loseContext();
  }
}

function createTexture(gl: WebGL2RenderingContext, width: number, height: number): WebGLTexture {
  const tex = gl.createTexture();
  if (!tex) throw new Error('Failed to create texture');
  gl.bindTexture(gl.TEXTURE_2D, tex);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, width, height, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
  gl.bindTexture(gl.TEXTURE_2D, null);
  return tex;
}

function compileShader(gl: WebGL2RenderingContext, type: number, src: string): WebGLShader {
  const shader = gl.createShader(type);
  if (!shader) throw new Error('Failed to create shader');
  gl.shaderSource(shader, src);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    const info = gl.getShaderInfoLog(shader);
    gl.deleteShader(shader);
    throw new Error(`Shader compile error: ${info}`);
  }
  return shader;
}

function linkProgram(gl: WebGL2RenderingContext, vs: WebGLShader, fs: WebGLShader): WebGLProgram {
  const program = gl.createProgram();
  if (!program) throw new Error('Failed to create program');
  gl.attachShader(program, vs);
  gl.attachShader(program, fs);
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    const info = gl.getProgramInfoLog(program);
    gl.deleteProgram(program);
    throw new Error(`Program link error: ${info}`);
  }
  return program;
}

const VERTEX_SRC = `#version 300 es
layout(location = 0) in vec2 aPos;
out vec2 vUv;
void main() {
  vUv = vec2(aPos.x * 0.5 + 0.5, 0.5 - aPos.y * 0.5);
  gl_Position = vec4(aPos, 0.0, 1.0);
}
`;

const FRAGMENT_SRC = `#version 300 es
precision mediump float;
in vec2 vUv;
uniform sampler2D uTex;
out vec4 outColor;
void main() {
  outColor = texture(uTex, vUv);
}
`;
