import fs from "node:fs";

const [graphPath, layersPath, outputPath] = process.argv.slice(2);
if (!graphPath || !layersPath || !outputPath) {
  console.error("usage: node ua-tour-prepare.mjs <graph> <layers> <output>");
  process.exit(1);
}
const graph = JSON.parse(fs.readFileSync(graphPath, "utf8"));
const layers = JSON.parse(fs.readFileSync(layersPath, "utf8"));
const fileLevelTypes = new Set(["file", "config", "document", "service", "pipeline", "resource", "table", "schema", "endpoint"]);
const nodes = graph.nodes.filter((node) => fileLevelTypes.has(node.type));
const input = {
  nodes,
  edges: graph.edges,
  layers: layers.map(({ id, name, description }) => ({ id, name, description })),
};
fs.writeFileSync(outputPath, JSON.stringify(input, null, 2) + "\n", "utf8");
console.log(JSON.stringify({ nodes: nodes.length, edges: graph.edges.length, layers: input.layers.length }));
