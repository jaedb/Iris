uniform sampler2D u_texture1;

void mainImage(
    out vec4 fragColor,
    in vec2 fragCoord,
    in vec2 resolution,
    in vec2 uv
) {
  fragColor = GskTexture(u_texture1, uv);
  fragColor.rgb = fragColor.rgb * fragColor.a;
}
