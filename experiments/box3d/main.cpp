// Independent experiment using the actual Box3D library. No copied solver code.
// Particle spheres and pairwise springs are NOT a continuum cloth/contact
// model.
#include <algorithm>
#include <atomic>
#include <box3d/box3d.h>
#include <box3d/collision.h>
#include <chrono>
#include <cmath>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <string>
#include <vector>
static std::atomic<bool> capacityExceeded = false;
static void logMessage(const char *message) {
  std::cerr << message << "\n";
  if (std::strstr(message, "triangle buffer capacity")) {
    capacityExceeded = true;
  }
}
struct Edge {
  int a, b;
  float length, k;
};
int main(int argc, char **argv) {
  if (argc != 5 && argc != 6) {
    std::cerr << "usage: clothcraft_box3d lattice.txt output-prefix steps "
                 "substeps [pull-vertex]\n";
    return 2;
  }
  std::ifstream in(argv[1]);
  int n, ne, nv, nt;
  float dt, damping;
  in >> n >> ne >> nv >> nt >> dt >> damping;
  if (!in || n <= 0 || ne < 0 || nv < 3 || nt < 1) {
    std::cerr << "Invalid lattice\n";
    return 2;
  }
  int steps = std::stoi(argv[3]), substeps = std::stoi(argv[4]);
  if (steps < 1 || substeps < 1) {
    return 2;
  }
  b3SetLogFcn(logMessage);
  std::string prefix = argv[2];
  if (std::filesystem::exists(prefix + "-steps.jsonl") ||
      std::filesystem::exists(prefix + "-states.jsonl")) {
    std::cerr << "Output exists; choose a fresh prefix\n";
    return 2;
  }
  b3WorldDef wd = b3DefaultWorldDef();
  in >> wd.gravity.x >> wd.gravity.y >> wd.gravity.z;
  wd.workerCount = 2;
  wd.enableSleep =
      false; // A sleeping lattice must not masquerade as a quiet solver.
  b3WorldId world = b3CreateWorld(&wd);
  std::vector<b3BodyId> bodies;
  std::vector<float> masses;
  std::vector<bool> pinned;
  constexpr float radius = 0.0015f;
  for (int i = 0; i < n; ++i) {
    b3BodyDef bd = b3DefaultBodyDef();
    float m;
    int pin;
    in >> bd.position.x >> bd.position.y >> bd.position.z >>
        bd.linearVelocity.x >> bd.linearVelocity.y >> bd.linearVelocity.z >>
        m >> pin;
    if (!in || !std::isfinite(m) || m <= 0 || (pin != 0 && pin != 1)) {
      std::cerr << "Invalid particle\n";
      return 2;
    }
    bd.type = pin ? b3_staticBody : b3_dynamicBody;
    bd.enableSleep = false;
    bd.linearDamping = damping;
    bd.motionLocks.angularX = bd.motionLocks.angularY =
        bd.motionLocks.angularZ = true;
    auto body = b3CreateBody(world, &bd);
    bodies.push_back(body);
    masses.push_back(m);
    pinned.push_back(pin != 0);
    b3ShapeDef sd = b3DefaultShapeDef();
    sd.density = m / (4.f / 3.f * B3_PI * radius * radius * radius);
    sd.baseMaterial.friction = 0;
    sd.baseMaterial.restitution = 0;
    b3Sphere sphere = {{0, 0, 0}, radius};
    b3CreateSphereShape(body, &sd, &sphere);
  }
  for (int j = 0; j < ne; ++j) {
    Edge e;
    in >> e.a >> e.b >> e.length >> e.k;
    if (!in || e.a < 0 || e.a >= n || e.b < 0 || e.b >= n || e.a == e.b ||
        !std::isfinite(e.k) || e.k <= 0 || !std::isfinite(e.length) ||
        e.length < 0) {
      std::cerr << "Invalid edge\n";
      return 2;
    }
    if (pinned[e.a] && pinned[e.b]) {
      continue;
    }
    b3DistanceJointDef jd = b3DefaultDistanceJointDef();
    jd.base.bodyIdA = bodies[e.a];
    jd.base.bodyIdB = bodies[e.b];
    jd.base.collideConnected = false;
    jd.length = e.length;
    jd.enableSpring = true;
    jd.dampingRatio = 0;
    float effective = masses[e.a] * masses[e.b] / (masses[e.a] + masses[e.b]);
    if (pinned[e.a]) {
      effective = masses[e.b];
    }
    if (pinned[e.b]) {
      effective = masses[e.a];
    }
    jd.hertz = std::sqrt(e.k / effective) / (2 * B3_PI);
    b3CreateDistanceJoint(world, &jd);
  }
  std::vector<b3Vec3> vertices(nv);
  std::vector<int32_t> triangles(3 * nt);
  for (auto &p : vertices) {
    in >> p.x >> p.y >> p.z;
  }
  for (auto &i : triangles) {
    in >> i;
    if (!in || i < 0 || i >= nv) {
      std::cerr << "Invalid avatar index\n";
      return 2;
    }
  }
  if (!in) {
    std::cerr << "Truncated lattice\n";
    return 2;
  }
  b3MeshDef md = {};
  md.vertices = vertices.data();
  md.indices = triangles.data();
  md.vertexCount = nv;
  md.triangleCount = nt;
  md.identifyEdges = true;
  auto mesh = b3CreateMesh(&md, nullptr, 0);
  if (!mesh) {
    return 3;
  }
  b3BodyDef bd = b3DefaultBodyDef();
  auto avatar = b3CreateBody(world, &bd);
  b3ShapeDef sd = b3DefaultShapeDef();
  sd.baseMaterial.friction = 0;
  sd.baseMaterial.restitution = 0;
  b3CreateMeshShape(avatar, &sd, mesh, {1, 1, 1});
  std::ofstream log(prefix + "-steps.jsonl"), trace(prefix + "-states.jsonl");
  log << std::setprecision(10);
  trace << std::setprecision(10);
  if (!log || !trace) {
    return 2;
  }
  std::vector<double> costs;
  int pullVertex = argc == 6 ? std::stoi(argv[5]) : -1;
  if (pullVertex >= n || pullVertex < -1) {
    return 2;
  }
  b3Pos anchor =
      pullVertex >= 0 ? b3Body_GetPosition(bodies[pullVertex]) : b3Pos{};
  b3BodyId handle = {};
  b3JointId grab = {};
  for (int step = 0; step < steps; ++step) {
    auto start = std::chrono::steady_clock::now();
    if (pullVertex >= 0 && step == 30) {
      auto def = b3DefaultBodyDef();
      def.type = b3_kinematicBody;
      def.position = anchor;
      handle = b3CreateBody(world, &def);
      auto jd = b3DefaultDistanceJointDef();
      jd.base.bodyIdA = handle;
      jd.base.bodyIdB = bodies[pullVertex];
      jd.enableSpring = true;
      jd.length = 0;
      jd.dampingRatio = 0;
      jd.hertz = std::sqrt(1000.f / masses[pullVertex]) / (2 * B3_PI);
      grab = b3CreateDistanceJoint(world, &jd);
    }
    if (pullVertex >= 0 && step >= 30 && step < 75) {
      b3Body_SetLinearVelocity(handle,
                               {0, 0, step < 60 ? 0.04f / (30 * dt) : 0.f});
    }
    if (pullVertex >= 0 && step == 75) {
      b3DestroyJoint(grab, true);
      b3DestroyBody(handle);
    }
    b3World_Step(world, dt, substeps);
    if (capacityExceeded) {
      std::cerr << "Contact capacity exceeded: run rejected at step " << step
                << "\n";
      return 5;
    }
    double kinetic = 0, mass = 0;
    std::vector<b3Pos> x;
    std::vector<b3Vec3> v;
    for (int i = 0; i < n; ++i) {
      auto p = b3Body_GetPosition(bodies[i]);
      auto u = b3Body_GetLinearVelocity(bodies[i]);
      if (!std::isfinite(p.x) || !std::isfinite(p.y) || !std::isfinite(p.z) ||
          !std::isfinite(u.x) || !std::isfinite(u.y) || !std::isfinite(u.z)) {
        std::cerr << "Nonfinite state\n";
        return 4;
      }
      x.push_back(p);
      v.push_back(u);
      kinetic += 0.5 * masses[i] * (u.x * u.x + u.y * u.y + u.z * u.z);
      mass += masses[i];
    }
    double ms = std::chrono::duration<double, std::milli>(
                    std::chrono::steady_clock::now() - start)
                    .count();
    costs.push_back(ms);
    log << "{\"step\":" << step << ",\"full_step_ms\":" << ms
        << ",\"kinetic_j\":" << kinetic
        << ",\"mass_rms_speed_m_s\":" << std::sqrt(2 * kinetic / mass) << "}\n";
    trace << "{\"x\":[";
    for (int i = 0; i < n; ++i) {
      if (i)
        trace << ",";
      trace << "[" << x[i].x << "," << x[i].y << "," << x[i].z << "]";
    }
    trace << "],\"v\":[";
    for (int i = 0; i < n; ++i) {
      if (i)
        trace << ",";
      trace << "[" << v[i].x << "," << v[i].y << "," << v[i].z << "]";
    }
    trace << "],\"time\":" << (step + 1) * double(dt) << "}\n";
  }
  b3DestroyWorld(world);
  b3DestroyMesh(mesh);
  if (costs.size() > 10) {
    costs.erase(costs.begin(), costs.begin() + 10);
  }
  std::sort(costs.begin(), costs.end());
  std::cout << "{\"backend\":\"box3d-lattice\",\"steps\":" << steps
            << ",\"substeps\":" << substeps
            << ",\"sleep\":false,\"median_ms\":" << costs[costs.size() / 2]
            << ",\"p95_ms\":"
            << costs[size_t(std::ceil(0.95 * (costs.size() - 1)))] << "}\n";
}
