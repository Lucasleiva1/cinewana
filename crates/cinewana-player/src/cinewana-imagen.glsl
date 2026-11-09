// CINE WANA — ajustes de imagen que no trae mpv de fábrica.
// Corre en la placa de video sobre la imagen final (ya escalada a la pantalla), así que no carga el
// procesador. Los valores se cambian en vivo con glsl-shader-opts sin recompilar nada. Con todo en 0
// la imagen sale idéntica a la original.

//!PARAM cw_brightness
//!DESC Brillo (multiplica la luz: el negro sigue negro)
//!TYPE DYNAMIC float
//!MINIMUM -1.0
//!MAXIMUM 1.0
0.0

//!PARAM cw_shadows
//!DESC Sombras
//!TYPE DYNAMIC float
//!MINIMUM -1.0
//!MAXIMUM 1.0
0.0

//!PARAM cw_highlights
//!DESC Luces
//!TYPE DYNAMIC float
//!MINIMUM -1.0
//!MAXIMUM 1.0
0.0

//!PARAM cw_temperature
//!DESC Temperatura (positivo: más cálida)
//!TYPE DYNAMIC float
//!MINIMUM -1.0
//!MAXIMUM 1.0
0.0

//!PARAM cw_sharpness
//!DESC Nitidez
//!TYPE DYNAMIC float
//!MINIMUM 0.0
//!MAXIMUM 1.0
0.0

//!HOOK OUTPUT
//!BIND HOOKED
//!DESC CINE WANA imagen

float cw_smooth(float edge0, float edge1, float x) {
    float t = clamp((x - edge0) / (edge1 - edge0), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

// Sombras: sube (o hunde) los tonos oscuros y medios-oscuros. Luces: aclara (o recupera) desde los
// medios hacia arriba, así se nota también en películas oscuras. El negro puro queda fijo para que las
// franjas negras no se pongan grises.
float cw_curve(float x) {
    float v = x;
    float dark = 1.0 - cw_smooth(0.0, 0.6, x);
    if (cw_shadows > 0.0) {
        v += 0.32 * cw_shadows * dark * cw_smooth(0.0, 0.04, x);
    } else if (cw_shadows < 0.0) {
        v -= 0.45 * (-cw_shadows) * dark * x;
    }
    if (cw_highlights > 0.0) {
        v += 0.38 * cw_highlights * cw_smooth(0.06, 0.6, x) * (1.0 - 0.6 * x);
    } else if (cw_highlights < 0.0) {
        v -= 0.38 * (-cw_highlights) * cw_smooth(0.2, 0.9, x) * x;
    }
    return clamp(v, 0.0, 1.0);
}

vec4 hook() {
    vec4 color = HOOKED_tex(HOOKED_pos);
    if (cw_sharpness > 0.0) {
        // Realce de bordes sobre la luz (no sobre el color, para no dejar bordes de colores).
        vec3 cross = HOOKED_texOff(vec2(1.0, 0.0)).rgb + HOOKED_texOff(vec2(-1.0, 0.0)).rgb
                   + HOOKED_texOff(vec2(0.0, 1.0)).rgb + HOOKED_texOff(vec2(0.0, -1.0)).rgb;
        vec3 corners = HOOKED_texOff(vec2(1.0, 1.0)).rgb + HOOKED_texOff(vec2(-1.0, 1.0)).rgb
                     + HOOKED_texOff(vec2(1.0, -1.0)).rgb + HOOKED_texOff(vec2(-1.0, -1.0)).rgb;
        vec3 blur = (color.rgb * 4.0 + cross * 2.0 + corners) / 16.0;
        float detail = dot(color.rgb - blur, vec3(0.2126, 0.7152, 0.0722));
        color.rgb += detail * cw_sharpness * 4.0;
    }
    color.rgb = clamp(color.rgb * (1.0 + cw_brightness * 0.5), 0.0, 1.0);
    color.rgb = vec3(cw_curve(color.r), cw_curve(color.g), cw_curve(color.b));
    float t = cw_temperature;
    vec3 tint = t >= 0.0
        ? vec3(1.0 + 0.10 * t, 1.0 + 0.015 * t, 1.0 - 0.16 * t)
        : vec3(1.0 + 0.14 * t, 1.0 + 0.01 * t, 1.0 - 0.10 * t);
    color.rgb = clamp(color.rgb * tint, 0.0, 1.0);
    return color;
}
