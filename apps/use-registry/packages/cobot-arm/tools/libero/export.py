"""Export libero_90 tasks 0 and 33 for the browser: a physics MJCF with no files, the
settled episode state with controller state, the reference expert trajectory, and the
render pack (every geom robosuite's camera draws, decimated and quantized)."""
import hashlib, importlib.metadata, io, json, os, re, struct, sys
import numpy as np
import mujoco
from scipy.spatial import ConvexHull
from PIL import Image
import fast_simplification
from common import TASKS, CAMERAS, EXPERT, INITS, OUT, make, goal_joints

REVISION = "8f1084e3132a39270c3a13ebe37270a43ece2a01"
FACE_BUDGET = int(os.environ.get("FACE_BUDGET", "70000"))
# A simplified mesh keeps its surface area within this share of the source; past
# it the simplifier tore holes or filled real ones (the microwave door's window),
# so the mesh gets more triangles instead.
AREA_TOLERANCE = 0.03
TEX_MAX = int(os.environ.get("TEX_MAX", "512"))
ARENA_BYTES = 16 << 20


def name(m, kind, i):
    return mujoco.mj_id2name(m, kind, i)


def physics_xml(xml, m):
    ref = mujoco.MjSpec.from_string(xml)
    ref.compile()
    spec = mujoco.MjSpec.from_string(xml)
    by_id = {r.id: g for r, g in zip(ref.geoms, spec.geoms, strict=True)}
    assert len(by_id) == m.ngeom
    for g in range(m.ngeom):
        assert by_id[g].name == (name(m, mujoco.mjtObj.mjOBJ_GEOM, g) or ""), g
    for r, body in zip(ref.bodies, spec.bodies, strict=True):
        assert r.name == body.name
        if r.id == 0:
            continue
        b = r.id
        body.explicitinertial = True
        body.mass = float(m.body_mass[b])
        body.ipos = m.body_ipos[b].tolist()
        body.iquat = m.body_iquat[b].tolist()
        body.inertia = m.body_inertia[b].tolist()
        body.fullinertia = [np.nan] * 6
    keep_mesh = {}
    for g in range(m.ngeom):
        geom = by_id[g]
        if m.geom_contype[g] == 0 and m.geom_conaffinity[g] == 0:
            spec.delete(geom)
            continue
        geom.material = ""
        if m.geom_type[g] == mujoco.mjtGeom.mjGEOM_MESH:
            geom.pos = m.geom_pos[g].tolist()
            geom.quat = m.geom_quat[g].tolist()
            keep_mesh[geom.meshname] = int(m.geom_dataid[g])
            assert name(m, mujoco.mjtObj.mjOBJ_MESH, int(m.geom_dataid[g])) == geom.meshname
    for mesh in list(spec.meshes):
        if mesh.name not in keep_mesh:
            spec.delete(mesh)
            continue
        i = keep_mesh[mesh.name]
        verts = m.mesh_vert[m.mesh_vertadr[i]:m.mesh_vertadr[i] + m.mesh_vertnum[i]]
        hull = verts[ConvexHull(verts).vertices]
        mesh_name = mesh.name
        spec.delete(mesh)
        fresh = spec.add_mesh()
        fresh.name = mesh_name
        fresh.uservert = hull.astype(float).ravel().tolist()
    for material in spec.materials:
        material.textures = [""] * len(material.textures)
    for texture in list(spec.textures):
        spec.delete(texture)
    spec.meshdir = ""
    spec.texturedir = ""
    # robosuite's njmax/nconmax of 5000 reserve a 1.26 GiB arena per MjData, past
    # what a browser's 2 GiB WASM heap can hold twice; the episodes peak near 0.13 MiB.
    spec.njmax = -1
    spec.nconmax = -1
    spec.memory = ARENA_BYTES
    out = spec.to_xml()
    assert 'file="' not in out
    assert 'njmax' not in out and 'nconmax' not in out
    return out


def world_hull(m, d, g):
    i = m.geom_dataid[g]
    verts = m.mesh_vert[m.mesh_vertadr[i]:m.mesh_vertadr[i] + m.mesh_vertnum[i]]
    w = verts @ d.geom_xmat[g].reshape(3, 3).T + d.geom_xpos[g]
    return w[ConvexHull(w).vertices]


def check_physics(xml_text, m, d):
    """The exported model must have the same dynamics-relevant arrays and the same
    world-space collision shapes as the robosuite model at the settled state."""
    p = mujoco.MjModel.from_xml_string(xml_text)
    q = mujoco.MjData(p)
    for field in ("nq", "nv", "nu", "nbody", "njnt", "neq", "ntendon", "nsite", "ncam"):
        assert getattr(p, field) == getattr(m, field), field
    for field in ("body_mass", "body_inertia", "body_ipos", "jnt_range", "jnt_axis", "dof_damping", "dof_armature",
                  "dof_frictionloss", "actuator_gear", "actuator_ctrlrange", "actuator_gainprm", "actuator_biasprm",
                  "site_pos", "cam_pos", "cam_quat", "cam_fovy", "opt.timestep", "opt.gravity"):
        a = getattr(p.opt, field[4:]) if field.startswith("opt.") else getattr(p, field)
        b = getattr(m.opt, field[4:]) if field.startswith("opt.") else getattr(m, field)
        assert np.allclose(a, b, atol=1e-9), field
    q.qpos[:] = d.qpos
    q.qvel[:] = d.qvel
    mujoco.mj_forward(p, q)
    names = {name(m, mujoco.mjtObj.mjOBJ_GEOM, g): g for g in range(m.ngeom)
             if m.geom_contype[g] or m.geom_conaffinity[g]}
    assert p.ngeom == len(names), (p.ngeom, len(names))
    worst = 0.0
    for gp in range(p.ngeom):
        g = names[name(p, mujoco.mjtObj.mjOBJ_GEOM, gp)] if name(p, mujoco.mjtObj.mjOBJ_GEOM, gp) else None
        if g is None:
            continue
        for field in ("geom_type", "geom_contype", "geom_conaffinity", "geom_condim", "geom_friction", "geom_solref",
                      "geom_solimp", "geom_margin", "geom_gap", "geom_priority", "geom_bodyid"):
            assert np.allclose(getattr(p, field)[gp], getattr(m, field)[g]), (field, gp)
        if m.geom_type[g] == mujoco.mjtGeom.mjGEOM_MESH:
            a, b = world_hull(m, d, g), world_hull(p, q, gp)
            dist = max(np.min(np.linalg.norm(a[:, None] - b[None], axis=2), axis=1).max(),
                       np.min(np.linalg.norm(b[:, None] - a[None], axis=2), axis=1).max())
            worst = max(worst, dist)
        else:
            assert np.allclose(p.geom_size[gp], m.geom_size[g])
            err = float(np.abs(q.geom_xpos[gp] - d.geom_xpos[g]).max())
            assert err < 1e-4, (name(m, mujoco.mjtObj.mjOBJ_GEOM, g), err, q.geom_xpos[gp], d.geom_xpos[g])
            worst = max(worst, err)
    return {"geoms": int(p.ngeom), "hull_max_deviation_m": float(worst)}


def mesh_soup(m, i):
    """Indexed triangles with one vertex per (position, normal, uv) corner."""
    va, vn = m.mesh_vertadr[i], m.mesh_vertnum[i]
    fa, fn = m.mesh_faceadr[i], m.mesh_facenum[i]
    verts = m.mesh_vert[va:va + vn]
    faces = m.mesh_face[fa:fa + fn]
    na = m.mesh_normaladr[i]
    normals = m.mesh_normal[na:na + m.mesh_normalnum[i]]
    fnorm = m.mesh_facenormal[fa:fa + fn]
    ta = m.mesh_texcoordadr[i]
    has_uv = ta >= 0 and m.mesh_texcoordnum[i] > 0
    uvs = m.mesh_texcoord[ta:ta + m.mesh_texcoordnum[i]] if has_uv else None
    ftex = m.mesh_facetexcoord[fa:fa + fn] if has_uv else None
    key = {}
    pos, nor, uv, tri = [], [], [], []
    for f in range(fn):
        for c in range(3):
            k = (faces[f][c], fnorm[f][c], ftex[f][c] if has_uv else -1)
            idx = key.get(k)
            if idx is None:
                idx = key[k] = len(pos)
                pos.append(verts[faces[f][c]])
                nor.append(normals[fnorm[f][c]])
                if has_uv:
                    uv.append(uvs[ftex[f][c]])
            tri.append(idx)
    return (np.array(pos, dtype=np.float64), np.array(nor, dtype=np.float64),
            np.array(uv, dtype=np.float64) if has_uv else None, np.array(tri, dtype=np.int64).reshape(-1, 3))


def decimate(pos, nor, uv, tri, target):
    """Simplify the welded surface, not the corner soup: a soup whose corners are
    all split is one island per triangle, and collapsing its edges deletes whole
    triangles. Edge collapses only re-point corners, so every surviving face is an
    original triangle and keeps that triangle's corner normals and texcoords."""
    if len(tri) <= target:
        return pos, nor, uv, tri
    welded, inverse = np.unique(pos, axis=0, return_inverse=True)
    wtri = inverse.reshape(-1)[tri]
    keep = (wtri[:, 0] != wtri[:, 1]) & (wtri[:, 1] != wtri[:, 2]) & (wtri[:, 0] != wtri[:, 2])
    tri, wtri = tri[keep], wtri[keep]
    reduction = 1 - target / len(wtri)
    _, _, collapses = fast_simplification.simplify(welded.astype(np.float32), wtri.astype(np.int32), reduction,
                                                   return_collapses=True)
    p2, _, mapping = fast_simplification.replay_simplification(welded.astype(np.float32), wtri.astype(np.int32),
                                                               collapses)
    mapped = np.asarray(mapping)[wtri]
    alive = (mapped[:, 0] != mapped[:, 1]) & (mapped[:, 1] != mapped[:, 2]) & (mapped[:, 0] != mapped[:, 2])
    _, first = np.unique(np.sort(mapped[alive], axis=1), axis=0, return_index=True)
    faces = np.flatnonzero(alive)[np.sort(first)]
    corners = tri[faces].reshape(-1)
    columns = [mapped[faces].reshape(-1, 1).astype(np.float64), nor[corners]]
    if uv is not None:
        columns.append(uv[corners])
    rows, out_tri = np.unique(np.hstack(columns), axis=0, return_inverse=True)
    vertex = rows[:, 0].astype(np.int64)
    return (p2[vertex].astype(np.float64), rows[:, 1:4], (rows[:, 4:6] if uv is not None else None),
            out_tri.reshape(-1, 3).astype(np.int64))


def surface_area(pos, tri):
    return float(np.linalg.norm(np.cross(pos[tri[:, 1]] - pos[tri[:, 0]], pos[tri[:, 2]] - pos[tri[:, 0]]), axis=1).sum() / 2)


def quantize(values, bits=16):
    lo, hi = values.min(0), values.max(0)
    span = np.where(hi - lo > 0, hi - lo, 1)
    q = np.round((values - lo) / span * (2 ** bits - 1)).astype(np.uint16)
    return q, lo.tolist(), span.tolist()


def scene_geoms(m, d):
    """The geoms robosuite's offscreen camera draws, with the colour and finish MuJoCo
    resolves for each: LIBERO renders with render_collision_mesh=False, which
    clears geom group 0 (robosuite environments/base.py)."""
    opt = mujoco.MjvOption()
    opt.geomgroup[0] = 0
    scn = mujoco.MjvScene(m, maxgeom=20000)
    cam = mujoco.MjvCamera()
    cam.type = mujoco.mjtCamera.mjCAMERA_FIXED
    cam.fixedcamid = mujoco.mj_name2id(m, mujoco.mjtObj.mjOBJ_CAMERA, CAMERAS["external"])
    mujoco.mjv_updateScene(m, d, opt, mujoco.MjvPerturb(), cam, mujoco.mjtCatBit.mjCAT_ALL, scn)
    out = {}
    for i in range(scn.ngeom):
        g = scn.geoms[i]
        if g.objtype == mujoco.mjtObj.mjOBJ_GEOM:
            out[int(g.objid)] = {"rgba": [float(v) for v in g.rgba], "emission": float(g.emission),
                                 "specular": float(g.specular), "shininess": float(g.shininess),
                                 "reflectance": float(g.reflectance)}
    assert all(m.geom_group[g] in (1, 2) for g in out), "a drawn geom is outside groups 1-2"
    return out


def render_pack(m, d, buf):
    """Every geom robosuite's offscreen camera draws, as MuJoCo's scene resolves it."""
    finish = scene_geoms(m, d)
    drawn = sorted(finish)
    meshes = sorted({int(m.geom_dataid[g]) for g in drawn if m.geom_type[g] == mujoco.mjtGeom.mjGEOM_MESH})
    soups = {i: mesh_soup(m, i) for i in meshes}
    total = sum(len(s[3]) for s in soups.values())
    scale = min(1.0, FACE_BUDGET / total)
    pack_meshes = {}
    faces_out = 0
    for i, (pos, nor, uv, tri) in soups.items():
        target = max(64, int(len(tri) * scale))
        source_area = surface_area(pos, tri)
        while True:
            out = decimate(pos, nor, uv, tri, target)
            if target >= len(tri) or abs(surface_area(out[0], out[3]) - source_area) <= AREA_TOLERANCE * source_area:
                break
            target = min(len(tri), target * 2)
        pos, nor, uv, tri = out
        faces_out += len(tri)
        qp, plo, pspan = quantize(pos)
        n = nor / np.maximum(np.linalg.norm(nor, axis=1, keepdims=True), 1e-12)
        qn = np.round(n * 127).astype(np.int8)
        entry = {"vertices": len(pos), "triangles": len(tri), "position": [buf.add(qp), plo, pspan],
                 "normal": buf.add(qn)}
        if uv is not None:
            qu, ulo, uspan = quantize(uv)
            entry["uv"] = [buf.add(qu), ulo, uspan]
        entry["index"] = buf.add(tri.astype(np.uint16 if len(pos) < 65536 else np.uint32))
        pack_meshes[name(m, mujoco.mjtObj.mjOBJ_MESH, i)] = entry
    textures = {}
    for t in range(m.ntex):
        w, h, c = int(m.tex_width[t]), int(m.tex_height[t]), int(m.tex_nchannel[t])
        data = np.asarray(m.tex_data[m.tex_adr[t]:m.tex_adr[t] + w * h * c], dtype=np.uint8).reshape(h, w, c)
        kind = int(m.tex_type[t])
        faces = [data] if kind == 0 else [data[k * w:(k + 1) * w] for k in range(h // w)] if h >= w * 6 else [data]
        out = []
        for face in faces:
            img = Image.fromarray(face[..., :3])
            if max(img.size) > TEX_MAX:
                img.thumbnail((TEX_MAX, TEX_MAX), Image.LANCZOS)
            b = io.BytesIO()
            img.save(b, format="JPEG", quality=82)
            out.append(buf.add_bytes(b.getvalue()))
        textures[t] = {"type": ["2d", "cube", "skybox"][kind], "faces": out}
    materials = {}
    for k in range(m.nmat):
        texid = int(m.mat_texid[k][1]) if m.mat_texid.ndim == 2 else int(m.mat_texid[k])
        if m.mat_texid.ndim == 2 and texid < 0:
            texid = int(max(m.mat_texid[k]))
        materials[k] = {"rgba": m.mat_rgba[k].tolist(), "emission": float(m.mat_emission[k]),
                        "specular": float(m.mat_specular[k]), "shininess": float(m.mat_shininess[k]),
                        "reflectance": float(m.mat_reflectance[k]), "texture": texid if texid >= 0 else None,
                        "texrepeat": m.mat_texrepeat[k].tolist(), "texuniform": bool(m.mat_texuniform[k])}
    geoms = []
    for g in drawn:
        item = {"body": name(m, mujoco.mjtObj.mjOBJ_BODY, int(m.geom_bodyid[g])), "type": int(m.geom_type[g]),
                "size": m.geom_size[g].tolist(), "pos": m.geom_pos[g].tolist(), "quat": m.geom_quat[g].tolist(),
                **finish[g], "material": int(m.geom_matid[g]) if m.geom_matid[g] >= 0 else None}
        if m.geom_type[g] == mujoco.mjtGeom.mjGEOM_MESH:
            item["mesh"] = name(m, mujoco.mjtObj.mjOBJ_MESH, int(m.geom_dataid[g]))
        geoms.append(item)
    lights = [{"pos": m.light_pos[k].tolist(), "dir": m.light_dir[k].tolist(), "body": int(m.light_bodyid[k]),
               "directional": bool(m.light_type[k] == 1) if hasattr(m, "light_type") else bool(m.light_directional[k]),
               "diffuse": m.light_diffuse[k].tolist(), "ambient": m.light_ambient[k].tolist(),
               "specular": m.light_specular[k].tolist(), "castshadow": bool(m.light_castshadow[k]),
               "active": bool(m.light_active[k]), "cutoff": float(m.light_cutoff[k]),
               "exponent": float(m.light_exponent[k]), "attenuation": m.light_attenuation[k].tolist()}
              for k in range(m.nlight)]
    visual = {"headlight": {"ambient": m.vis.headlight.ambient.tolist(), "diffuse": m.vis.headlight.diffuse.tolist(),
                            "specular": m.vis.headlight.specular.tolist(), "active": bool(m.vis.headlight.active)},
              "znear": float(m.vis.map.znear), "zfar": float(m.vis.map.zfar), "extent": float(m.stat.extent)}
    return {"geoms": geoms, "meshes": pack_meshes, "textures": textures, "materials": materials, "lights": lights,
            "visual": visual, "triangles": faces_out, "source_triangles": total}


class Buffer:
    def __init__(self):
        self.parts, self.size = [], 0

    def add(self, array):
        return self.add_bytes(np.ascontiguousarray(array).tobytes(), str(array.dtype), list(array.shape))

    def add_bytes(self, data, dtype="bytes", shape=None):
        pad = (-self.size) % 8
        if pad:
            self.parts.append(b"\0" * pad)
            self.size += pad
        ref = {"offset": self.size, "bytes": len(data), "dtype": dtype}
        if shape is not None:
            ref["shape"] = shape
        self.parts.append(data)
        self.size += len(data)
        return ref

    def bytes(self):
        return b"".join(self.parts)


def camera_calibration(env, size):
    from robosuite.utils.camera_utils import get_camera_intrinsic_matrix, get_camera_extrinsic_matrix
    return {view: {"camera": cam, "intrinsics": get_camera_intrinsic_matrix(env.sim, cam, size, size).tolist(),
                   "camera_to_world": get_camera_extrinsic_matrix(env.sim, cam).tolist()}
            for view, cam in CAMERAS.items()}


def reference(task_id, eef_site):
    """Replay the expert on robosuite and record the per-step result for parity checks."""
    task, env, obs = make(task_id)
    m, d = env.sim.model._model, env.sim.data._data
    goal = goal_joints(env, task_id)
    adr = [int(m.jnt_qposadr[mujoco.mj_name2id(m, mujoco.mjtObj.mjOBJ_JOINT, g["joint"])]) for g in goal]
    actions = json.load(open(EXPERT))
    actions = next(a for a in actions if a["task_id"] == task_id)["actions"]
    site = mujoco.mj_name2id(m, mujoco.mjtObj.mjOBJ_SITE, eef_site)
    robot = env.robots[0]
    arm = [int(i) for i in robot.controller.qpos_index]
    fingers = [int(i) for i in robot._ref_gripper_joint_pos_indexes]
    steps = []
    for a in actions:
        env.step(np.array(a, dtype=float))
        steps.append({"eef": d.site_xpos[site].tolist(), "arm_qpos": d.qpos[arm].tolist(),
                      "finger_qpos": d.qpos[fingers].tolist(), "goal_qpos": [float(d.qpos[k]) for k in adr],
                      "success": bool(env.check_success())})
    env.close()
    return {"actions": actions, "steps": steps}


def settled_state(d):
    return {"time": float(d.time), "qpos": d.qpos.tolist(), "qvel": d.qvel.tolist(), "act": d.act.tolist(),
            "ctrl": d.ctrl.tolist(), "qacc_warmstart": d.qacc_warmstart.tolist()}


def settled_inits(task_id, phys):
    """Each of the first INITS init states, settled as robosuite settles them, with the
    controller state robosuite holds there. The exported model must reproduce every one."""
    inits = []
    for index in range(INITS):
        _, env, obs = make(task_id, init_index=index)
        m, d = env.sim.model._model, env.sim.data._data
        c = env.robots[0].controller
        inits.append({
            "index": index,
            "settled_state_sha256": hashlib.sha256(np.asarray(env.get_sim_state(), dtype="<f8").tobytes()).hexdigest(),
            "state": settled_state(d),
            "goal_pos": np.asarray(c.goal_pos).tolist(), "goal_ori": np.asarray(c.goal_ori).tolist(),
            "initial_joint": np.asarray(c.initial_joint).tolist(),
            "gripper_action": np.asarray(env.robots[0].gripper.current_action).tolist(),
            "observation": {"tcp": obs["robot0_eef_pos"].tolist(), "quat_xyzw": obs["robot0_eef_quat"].tolist(),
                            "gripper_qpos": obs["robot0_gripper_qpos"].tolist()},
            "initial_success": bool(env.check_success()),
            "physics_check": check_physics(phys, m, d),
        })
        env.close()
    return inits


def export(task_id):
    task, env, obs = make(task_id, cameras=True)
    m, d = env.sim.model._model, env.sim.data._data
    robot = env.robots[0]
    c = robot.controller
    xml = env.sim.model.get_xml()
    phys = physics_xml(xml, m)
    physics_check = check_physics(phys, m, d)
    images = {}
    for view, cam in CAMERAS.items():
        images[view] = obs[cam + "_image"][::-1]
    gripper_ids = [mujoco.mj_name2id(m, mujoco.mjtObj.mjOBJ_ACTUATOR, a) for a in robot.gripper.actuators]
    episode = {
        "format": "aster-libero-episode-v1",
        "benchmark": "LIBERO", "libero_revision": REVISION, "suite": "libero_90", "task_id": task_id,
        "task_name": task.name, "language": task.language, "init_index": 0, "seed": 0, "settle_steps": 10,
        "versions": {k: importlib.metadata.version(k) for k in ("mujoco", "robosuite", "numpy", "torch")},
        "settled_state_sha256": hashlib.sha256(np.asarray(env.get_sim_state(), dtype="<f8").tobytes()).hexdigest(),
        "state": settled_state(d),
        "controller": {
            "type": "OSC_POSE", "control_hz": 20, "substeps": int(round(env.env.control_timestep / env.env.model_timestep)),
            "eef_site": c.eef_name, "eef_body": robot.robot_model.eef_name, "kp": c.kp.tolist(), "kd": c.kd.tolist(), "nullspace_kp": 10.0,
            "input_min": np.asarray(c.input_min).tolist(), "input_max": np.asarray(c.input_max).tolist(),
            "output_min": np.asarray(c.output_min).tolist(), "output_max": np.asarray(c.output_max).tolist(),
            "qpos_index": [int(i) for i in c.qpos_index], "qvel_index": [int(i) for i in c.qvel_index],
            "arm_actuators": [int(i) for i in robot._ref_joint_actuator_indexes],
            "torque_low": robot.torque_limits[0].tolist(), "torque_high": robot.torque_limits[1].tolist(),
            "goal_pos": np.asarray(c.goal_pos).tolist(), "goal_ori": np.asarray(c.goal_ori).tolist(),
            "initial_joint": np.asarray(c.initial_joint).tolist(),
            "gripper_actuators": gripper_ids, "gripper_speed": float(robot.gripper.speed),
            "finger_qpos_index": [int(i) for i in robot._ref_gripper_joint_pos_indexes],
            "gripper_action": np.asarray(robot.gripper.current_action).tolist(),
            "gripper_ctrlrange": m.actuator_ctrlrange[gripper_ids].tolist(),
        },
        "success": {"predicate": "Close", "joints": goal_joints(env, task_id)},
        "cameras": {"size": 384, "transform": "vertical flip to top-left origin", **camera_calibration(env, 384)},
        "observation": {"tcp": obs["robot0_eef_pos"].tolist(), "quat_xyzw": obs["robot0_eef_quat"].tolist(),
                        "gripper_qpos": obs["robot0_gripper_qpos"].tolist()},
        "physics_check": physics_check,
        "initial_success": bool(env.check_success()),
    }
    buf = Buffer()
    render = render_pack(m, d, buf)
    env.close()
    episode["reference"] = reference(task_id, c.eef_name)
    episode["inits"] = settled_inits(task_id, phys)
    assert episode["inits"][0]["settled_state_sha256"] == episode["settled_state_sha256"]
    folder = os.path.join(OUT, TASKS[task_id])
    os.makedirs(folder, exist_ok=True)
    open(os.path.join(folder, "physics.xml"), "w").write(phys)
    json.dump(episode, open(os.path.join(folder, "episode.json"), "w"))
    json.dump(render, open(os.path.join(folder, "render.json"), "w"))
    open(os.path.join(folder, "render.bin"), "wb").write(buf.bytes())
    for view, rgb in images.items():
        Image.fromarray(rgb).save(os.path.join(folder, f"robosuite-{view}.png"))
    print(TASKS[task_id], "physics", len(phys), "bytes", physics_check, "render triangles", render["triangles"],
          "of", render["source_triangles"], "bin", buf.size, "success at", next(
              (i + 1 for i, s in enumerate(episode["reference"]["steps"]) if s["success"]), None))


if __name__ == "__main__":
    for t in TASKS:
        export(t)
