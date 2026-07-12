#!/usr/bin/env node
const fs = require("fs");

const [scanPath, graphPath, layersPath, tourPath, outputPath, gitCommitHash] = process.argv.slice(2);
const scan = JSON.parse(fs.readFileSync(scanPath, "utf8"));
const graph = JSON.parse(fs.readFileSync(graphPath, "utf8"));
const layersRaw = JSON.parse(fs.readFileSync(layersPath, "utf8"));
const tourRaw = JSON.parse(fs.readFileSync(tourPath, "utf8"));
const layers = Array.isArray(layersRaw) ? layersRaw : layersRaw.layers;
const tour = (Array.isArray(tourRaw) ? tourRaw : tourRaw.steps).slice().sort((a, b) => a.order - b.order);

if (!Array.isArray(graph.nodes) || !Array.isArray(graph.edges)) throw new Error("assembled graph nodes/edges missing");
if (!Array.isArray(layers) || !Array.isArray(tour)) throw new Error("layers/tour must be arrays");

const nodeIds = new Set(graph.nodes.map((node) => node.id));
const fileTypes = new Set(["file", "config", "document", "service", "pipeline", "table", "schema", "resource", "endpoint"]);
const fileNodeIds = graph.nodes.filter((node) => fileTypes.has(node.type)).map((node) => node.id);
const assigned = new Map();

for (const layer of layers) {
  for (const field of ["id", "name", "description"]) {
    if (typeof layer[field] !== "string" || !layer[field]) throw new Error(`layer missing ${field}`);
  }
  if (!Array.isArray(layer.nodeIds) || layer.nodeIds.length === 0) throw new Error(`layer ${layer.id} has no nodeIds`);
  for (const id of layer.nodeIds) {
    if (!nodeIds.has(id)) throw new Error(`layer ${layer.id} references missing node ${id}`);
    if (assigned.has(id)) throw new Error(`node ${id} assigned to multiple layers`);
    assigned.set(id, layer.id);
  }
}
for (const id of fileNodeIds) {
  if (!assigned.has(id)) throw new Error(`file node ${id} is not assigned to a layer`);
}

tour.forEach((step, index) => {
  if (step.order !== index + 1) throw new Error(`tour order must be sequential at index ${index}`);
  for (const field of ["title", "description"]) {
    if (typeof step[field] !== "string" || !step[field]) throw new Error(`tour step ${index + 1} missing ${field}`);
  }
  if (!Array.isArray(step.nodeIds) || step.nodeIds.length === 0) throw new Error(`tour step ${index + 1} has no nodeIds`);
  for (const id of step.nodeIds) {
    if (!nodeIds.has(id)) throw new Error(`tour step ${index + 1} references missing node ${id}`);
  }
});

const output = {
  version: "1.0.0",
  project: {
    name: scan.name,
    languages: scan.languages,
    frameworks: scan.frameworks,
    description: scan.description,
    analyzedAt: new Date().toISOString(),
    gitCommitHash,
  },
  nodes: graph.nodes,
  edges: graph.edges,
  layers,
  tour,
};

fs.writeFileSync(outputPath, JSON.stringify(output, null, 2) + "\n");
process.stdout.write(`assembled nodes=${output.nodes.length} edges=${output.edges.length} layers=${layers.length} tour=${tour.length}\n`);
