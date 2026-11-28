// One pipeline draws the whole app: every shape is an instance of one quad, and the fragment
// shader works out its edge from a signed distance, so corners, rings and soft shadows stay
// smooth at any size. Adapted from Feel the Beat's renderer, by way of Tally RS.

struct Globals {
    // Size of the target in pixels.
    screen: vec2<f32>,
    // x: 1 when the target converts to sRGB on write, so colours must be made linear first.
    flags: vec2<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;
@group(0) @binding(3) var images: texture_2d<f32>;

const KIND_BOX: u32 = 0u;
const KIND_GLYPH: u32 = 1u;
const KIND_ARC: u32 = 2u;
const KIND_IMAGE: u32 = 5u;
const FLAG_HGRAD: u32 = 64u;
const FLAG_PLAIN: u32 = 128u;
const FLAG_BUTT: u32 = 256u;
const FLAG_HARD: u32 = 512u;

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) grad: f32,
    @location(3) @interpolate(flat) half: vec2<f32>,
    @location(4) @interpolate(flat) c0: vec4<f32>,
    @location(5) @interpolate(flat) c1: vec4<f32>,
    @location(6) @interpolate(flat) p: vec4<f32>,
    @location(7) @interpolate(flat) e: vec4<f32>,
};

@vertex
fn vs(
    @builtin(vertex_index) vi: u32,
    @location(0) rect: vec4<f32>,
    @location(1) uv: vec4<f32>,
    @location(2) c0: vec4<f32>,
    @location(3) c1: vec4<f32>,
    @location(4) p: vec4<f32>,
    @location(5) e: vec4<f32>,
) -> VOut {
    var out: VOut;
    let corner = vec2<f32>(f32(vi & 1u), f32(vi >> 1u));
    let bits = u32(p.w);
    let plain = (bits & (FLAG_PLAIN | FLAG_HARD)) != 0u;
    // Room for the antialiased edge and any soft reach.
    let margin = select(p.z + 1.0, 0.0, plain);
    let size = rect.zw + vec2<f32>(2.0 * margin);
    let local = (corner - vec2<f32>(0.5)) * size;
    let world = rect.xy + rect.zw * 0.5 + local;
    out.pos = vec4<f32>(world.x / g.screen.x * 2.0 - 1.0, 1.0 - world.y / g.screen.y * 2.0, 0.0, 1.0);
    out.local = local;
    out.uv = mix(uv.xy, uv.zw, corner);
    let t = local / max(rect.zw, vec2<f32>(0.0001)) + vec2<f32>(0.5);
    out.grad = clamp(select(t.y, t.x, (bits & FLAG_HGRAD) != 0u), 0.0, 1.0);
    out.half = rect.zw * 0.5;
    if g.flags.x > 0.5 {
        out.c0 = vec4<f32>(pow(c0.rgb, vec3<f32>(2.2)), c0.a);
        out.c1 = vec4<f32>(pow(c1.rgb, vec3<f32>(2.2)), c1.a);
    } else {
        out.c0 = c0;
        out.c1 = c1;
    }
    out.p = p;
    out.e = e;
    return out;
}

fn sd_box(p: vec2<f32>, b: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - b + vec2<f32>(r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

@fragment
fn fs(in: VOut) -> @location(0) vec4<f32> {
    let bits = u32(in.p.w);
    if (bits & FLAG_PLAIN) != 0u {
        let flat_colour = mix(in.c0, in.c1, in.grad);
        return vec4<f32>(flat_colour.rgb * flat_colour.a, flat_colour.a);
    }
    if (bits & FLAG_HARD) != 0u {
        let texel = textureSampleLevel(images, atlas_sampler, in.uv, 0.0);
        let c = vec4<f32>(texel.rgb * in.c0.rgb, texel.a * in.c0.a);
        return vec4<f32>(c.rgb * c.a, c.a);
    }
    let kind = bits & 15u;
    let radius = in.p.x;
    let stroke = in.p.y;
    let soft = in.p.z;
    var colour = mix(in.c0, in.c1, in.grad);

    if kind == KIND_GLYPH {
        let a = textureSampleLevel(atlas, atlas_sampler, in.uv, 0.0).r * colour.a;
        if a < 0.004 {
            discard;
        }
        return vec4<f32>(colour.rgb * a, a);
    }

    var d: f32;
    if kind == KIND_ARC {
        // A stroke along a circle filling the quad, from angle e.y to e.z (radians clockwise
        // from the top).
        let tau = 6.2831853;
        let q = in.local;
        let arc_r = min(in.half.x, in.half.y) - stroke * 0.5;
        let dist = length(q);
        let ring = abs(dist - arc_r) - stroke * 0.5;
        if ring > 1.5 {
            discard;
        }
        var ang = atan2(q.x, -q.y);
        if ang < 0.0 {
            ang += tau;
        }
        let sweep = max(in.e.z - in.e.y, 0.0001);
        let rel = ang - in.e.y - tau * floor((ang - in.e.y) / tau);
        if rel <= sweep {
            d = ring;
        } else if (bits & FLAG_BUTT) != 0u {
            let past = min(rel - sweep, tau - rel);
            d = max(ring, dist * sin(min(past, 1.5707)));
        } else {
            let end0 = arc_r * vec2<f32>(sin(in.e.y), -cos(in.e.y));
            let end1 = arc_r * vec2<f32>(sin(in.e.z), -cos(in.e.z));
            d = min(length(q - end0), length(q - end1)) - stroke * 0.5;
        }
    } else {
        d = sd_box(in.local, in.half, min(radius, min(in.half.x, in.half.y)));
        if stroke > 0.0 {
            d = abs(d + stroke * 0.5) - stroke * 0.5;
        }
    }
    let aa = max(fwidth(d), 0.0001) * 0.75;
    var alpha = 1.0 - smoothstep(-aa, aa, d);
    if soft > 0.0 {
        // A soft shadow: fades out over `soft` pixels past the edge.
        let fall = 1.0 - clamp((d + soft * 0.35) / soft, 0.0, 1.0);
        alpha = fall * fall;
    }
    if kind == KIND_IMAGE {
        let texel = textureSampleLevel(images, atlas_sampler, in.uv, 0.0);
        colour = vec4<f32>(texel.rgb * in.c0.rgb, texel.a * in.c0.a);
    }
    let a = colour.a * alpha;
    if a < 0.004 {
        discard;
    }
    return vec4<f32>(colour.rgb * a, a);
}
