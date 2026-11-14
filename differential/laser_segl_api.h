/* GitHub #169: the calls W3DLaserDraw's stand-in SegmentedLineClass (differential/wrap_laser_draw.cpp, namespace ld,
** compiled on the game engine's headers) forwards to the REAL SegmentedLineClass (differential/wrap_segline_obj.cpp,
** compiled VERBATIM on the WW3D2 headers; the two sets of headers do not mix in one translation unit). Plain C types
** only. The table lives in the laser library (g_ld_segl_api); ref_laser_segl_bind() in the segline object library fills
** it, so the laser library has no link-time reference to the segline libraries (they are linked before it). */
#ifndef LASER_SEGL_API_H
#define LASER_SEGL_API_H

struct LdSeglApi
{
	/* the WW3D clock the next lines are made at (SegLineRendererClass's constructor reads WW3D::Get_Sync_Time) */
	void (*clock)(unsigned sync);
	/* NEW SegmentedLineClass / REF_PTR_RELEASE */
	void *(*make)(void);
	void (*release)(void *line);
	/* the setters W3DLaserDraw calls */
	void (*set_texture)(void *line, int has_texture);
	void (*set_shader_additive)(void *line); /* Set_Shader(ShaderClass::_PresetAdditiveShader) */
	void (*set_width)(void *line, float width);
	void (*set_color)(void *line, float r, float g, float b);
	void (*set_uv_offset_rate)(void *line, float u, float v);
	void (*set_texture_mapping_mode)(void *line, int mode);
	void (*set_visible)(void *line, int onoff);
	void (*set_texture_tile_factor)(void *line, float factor);
	void (*set_points)(void *line, unsigned n, const float *xyz);
	/* the line as its getters read it: see wrap_segline_obj.cpp's ls_state */
	int (*state)(void *line, int *out, int cap);
	/* Render at `sync` with the view's three rows (12 floats): the chunks' words (wrap_segline.cpp's) */
	int (*render)(void *line, unsigned sync, const float *view, int *out, int cap);
};

#ifdef __cplusplus
extern "C" {
#endif
extern struct LdSeglApi g_ld_segl_api;
#ifdef __cplusplus
}
#endif

#endif
