import fs from "node:fs";
import path from "node:path";

const [inputPath, outputPath] = process.argv.slice(2);
if (!inputPath || !outputPath) {
  console.error("usage: node ua-tour-analyze.mjs <input> <output>");
  process.exit(1);
}

try {
  const input = JSON.parse(fs.readFileSync(inputPath, "utf8"));
  if (!Array.isArray(input.nodes) || !Array.isArray(input.edges) || !Array.isArray(input.layers)) {
    throw new Error("input must contain nodes, edges, and layers arrays");
  }
  const nodes = [...input.nodes].sort((a, b) => a.id.localeCompare(b.id));
  const nodeById = new Map(nodes.map((node) => [node.id, node]));
  const fanIn = new Map(nodes.map((node) => [node.id, 0]));
  const fanOut = new Map(nodes.map((node) => [node.id, 0]));
  for (const edge of input.edges) {
    if (nodeById.has(edge.source)) fanOut.set(edge.source, fanOut.get(edge.source) + 1);
    if (nodeById.has(edge.target)) fanIn.set(edge.target, fanIn.get(edge.target) + 1);
  }
  const rank = (counts, label) => nodes
    .map((node) => ({ id: node.id, [label]: counts.get(node.id), name: node.name }))
    .sort((a, b) => b[label] - a[label] || a.id.localeCompare(b.id))
    .slice(0, 20);
  const fanInRanking = rank(fanIn, "fanIn");
  const fanOutRanking = rank(fanOut, "fanOut");

  const topFanOut = new Set([...nodes]
    .sort((a, b) => fanOut.get(b.id) - fanOut.get(a.id) || a.id.localeCompare(b.id))
    .slice(0, Math.max(1, Math.ceil(nodes.length * 0.10)))
    .map((node) => node.id));
  const lowFanIn = new Set([...nodes]
    .sort((a, b) => fanIn.get(a.id) - fanIn.get(b.id) || a.id.localeCompare(b.id))
    .slice(0, Math.max(1, Math.ceil(nodes.length * 0.25)))
    .map((node) => node.id));
  const entryNames = /^(index\.(ts|js)|main\.(ts|js|go|py|rs|cpp|c)|app\.(ts|js|py)|server\.(ts|js)|mod\.rs|manage\.py|wsgi\.py|asgi\.py|run\.py|__main__\.py|Application\.java|Main\.java|Program\.cs|config\.ru|index\.php|App\.swift|Application\.kt)$/;
  const entryPointCandidates = nodes.map((node) => {
    const filePath = node.filePath ?? "";
    const name = path.posix.basename(filePath || node.name || "");
    let score = 0;
    if (node.type === "file") {
      if (entryNames.test(name)) score += 3;
      const depth = filePath.split("/").filter(Boolean).length;
      if (depth <= 2) score += 1;
      if (topFanOut.has(node.id)) score += 1;
      if (lowFanIn.has(node.id)) score += 1;
    } else if (node.type === "document") {
      if (filePath === "README.md") score += 5;
      else if (filePath.endsWith(".md") && !filePath.includes("/")) score += 2;
    }
    return { id: node.id, score, name: node.name, summary: node.summary ?? "" };
  }).filter((item) => item.score > 0)
    .sort((a, b) => b.score - a.score || a.id.localeCompare(b.id))
    .slice(0, 5);

  const codeEntry = entryPointCandidates.find((item) => nodeById.get(item.id)?.type === "file")
    ?? nodes.find((node) => node.type === "file" && entryNames.test(node.name));
  const traversable = new Map(nodes.map((node) => [node.id, []]));
  for (const edge of input.edges) {
    if ((edge.type === "imports" || edge.type === "calls") && nodeById.has(edge.source) && nodeById.has(edge.target)) {
      traversable.get(edge.source).push(edge.target);
    }
  }
  for (const targets of traversable.values()) targets.sort((a, b) => a.localeCompare(b));
  const order = [];
  const depthMap = {};
  const byDepth = {};
  if (codeEntry) {
    const queue = [codeEntry.id];
    depthMap[codeEntry.id] = 0;
    while (queue.length) {
      const current = queue.shift();
      const depth = depthMap[current];
      order.push(current);
      (byDepth[String(depth)] ??= []).push(current);
      for (const target of traversable.get(current) ?? []) {
        if (depthMap[target] !== undefined) continue;
        depthMap[target] = depth + 1;
        queue.push(target);
      }
    }
  }

  const inventory = (types) => nodes.filter((node) => types.has(node.type)).map((node) => ({ id:node.id, name:node.name, type:node.type, summary:node.summary ?? "" }));
  const nonCodeFiles = {
    documentation: inventory(new Set(["document"])),
    infrastructure: inventory(new Set(["service", "pipeline", "resource"])),
    data: inventory(new Set(["table", "schema", "endpoint"])),
    config: inventory(new Set(["config"])),
  };

  const relationship = new Set(input.edges
    .filter((edge) => (edge.type === "imports" || edge.type === "calls") && nodeById.has(edge.source) && nodeById.has(edge.target))
    .map((edge) => `${edge.source}\u0000${edge.target}`));
  const clusterMap = new Map();
  for (const key of relationship) {
    const [left, right] = key.split("\u0000");
    if (!relationship.has(`${right}\u0000${left}`) || left >= right) continue;
    const members = new Set([left, right]);
    let changed = true;
    while (changed && members.size < 5) {
      changed = false;
      for (const candidate of nodes.map((node) => node.id)) {
        if (members.has(candidate)) continue;
        const links = [...members].filter((member) => relationship.has(`${candidate}\u0000${member}`) || relationship.has(`${member}\u0000${candidate}`)).length;
        if (links >= 2) { members.add(candidate); changed = true; if (members.size === 5) break; }
      }
    }
    const list = [...members].sort();
    const clusterKey = list.join("\u0000");
    let edgeCount = 0;
    for (const source of list) for (const target of list) if (relationship.has(`${source}\u0000${target}`)) edgeCount += 1;
    clusterMap.set(clusterKey, { nodes:list, edgeCount });
  }
  const clusters = [...clusterMap.values()].sort((a, b) => b.edgeCount - a.edgeCount || a.nodes.join().localeCompare(b.nodes.join())).slice(0, 10);

  const nodeSummaryIndex = Object.fromEntries(nodes.map((node) => [node.id, { name:node.name, type:node.type, summary:node.summary ?? "" }]));
  const result = {
    scriptCompleted:true,
    entryPointCandidates,
    fanInRanking,
    fanOutRanking,
    bfsTraversal:{ startNode:codeEntry?.id ?? null, order, depthMap, byDepth },
    nonCodeFiles,
    clusters,
    layers:{ count:input.layers.length, list:input.layers.map(({ id, name, description }) => ({ id, name, description })) },
    nodeSummaryIndex,
    totalNodes:nodes.length,
    totalEdges:input.edges.length,
  };
  fs.writeFileSync(outputPath, JSON.stringify(result, null, 2) + "\n", "utf8");
  console.log(JSON.stringify({ nodes:result.totalNodes, edges:result.totalEdges, start:result.bfsTraversal.startNode, reached:order.length, clusters:clusters.length }));
} catch (error) {
  console.error(error instanceof Error ? error.stack : String(error));
  process.exit(1);
}
