// Analytic, transparent halo. The CPU supplies a circular damped audio profile.
// All distances are measured against the ring itself; there are no polygon edges.
in vec2 v_uv;
out vec4 out_color;
uniform vec2 u_canvas_size;
uniform vec2 u_canvas_offset;
uniform vec2 u_center;
uniform float u_half_size;
uniform float u_profile[128];
uniform vec3 u_color;
uniform float u_opacity;
uniform float u_glow;
uniform float u_thickness;
uniform float u_rotation;

const float TAU = 6.28318530718;

float band(int index) {
    return u_profile[(index + 128) % 128];
}

// Periodic Catmull-Rom interpolation, including a matching derivative across
// the seam. Clamp the radius so a spring overshoot cannot tear the silhouette.
vec2 profile(float angle) {
    float x = fract(angle / TAU) * 128.0;
    int i = int(floor(x));
    float t = fract(x);
    float a = band(i - 1);
    float b = band(i);
    float c = band(i + 1);
    float d = band(i + 2);
    float p = -a + c;
    float q = 2.0 * a - 5.0 * b + 4.0 * c - d;
    float s = -a + 3.0 * b - 3.0 * c + d;
    float value = 0.5 * (2.0 * b + p * t + q * t * t + s * t * t * t);
    float slope = 0.5 * (p + 2.0 * q * t + 3.0 * s * t * t) * 128.0 / TAU;
    return vec2(clamp(value, -0.025, 0.19), slope);
}

void main() {
    vec2 p = (v_uv * u_canvas_size + u_canvas_offset - u_center) / u_half_size;
    float r = length(p);
    float angle = atan(p.y, p.x) - u_rotation;
    vec2 displacement = profile(angle);
    float radius = 0.72 + displacement.x;
    float radial_distance = r - radius;
    float distance_to_ring = abs(radial_distance)
        / sqrt(1.0 + pow(displacement.y / max(radius, 0.1), 2.0));
    float aa = max(fwidth(radial_distance), 0.5 / u_half_size);
    float width = max(u_thickness / u_half_size * 0.5, aa * 0.65);
    float core = 1.0 - smoothstep(width, width + aa, distance_to_ring);
    float near_glow = exp(-pow(distance_to_ring / (width + 0.012), 2.0));
    float halo = exp(-pow(distance_to_ring / (0.022 + u_glow * 0.085), 2.0));
    // A soft luminous membrane inside outward waves preserves the liquid look
    // without filling the middle of the ring or adding an artificial inner rim.
    float membrane = smoothstep(0.714, 0.734, r)
        * (1.0 - smoothstep(radius - 0.008, radius + aa, r))
        * smoothstep(0.004, 0.045, displacement.x) * 0.48;
    float core_weight = core * 0.98;
    float color_weight = near_glow * u_glow * 0.38
        + halo * u_glow * 0.28 + membrane;
    float alpha = clamp(core_weight + color_weight, 0.0, 1.0) * u_opacity;
    vec3 hot_color = mix(u_color, vec3(1.0), 0.84);
    vec3 straight_color = (hot_color * core_weight + u_color * color_weight)
        / max(core_weight + color_weight, 0.00001);
    out_color = vec4(straight_color * alpha, alpha);
}
