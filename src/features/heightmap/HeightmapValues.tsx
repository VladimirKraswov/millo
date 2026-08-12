import type { Heightmap } from "../../shared/heightmap";
import { heightColor, heightmapMatrix } from "./heightmapModel";

export function HeightmapValues({ map }: { readonly map?: Heightmap }) {
  if (!map) {
    return (
      <div className="heightmap-values-empty">
        <strong>Значений пока нет</strong>
        <span>После касания точки здесь появится её рабочая Z.</span>
      </div>
    );
  }
  const matrix = heightmapMatrix(map);
  const values = matrix.flatMap((row) => row.filter((value): value is number => value !== undefined));
  const minimum = values.length ? Math.min(...values) : 0;
  const maximum = values.length ? Math.max(...values) : 0;
  const reference = values[0] ?? 0;
  const xCoordinates = Array.from({ length: map.plan.request.columns }, (_, column) =>
    map.plan.request.originXmm + map.plan.spacing.xMm * column,
  );
  const yCoordinates = Array.from({ length: map.plan.request.rows }, (_, row) =>
    map.plan.request.originYmm + map.plan.spacing.yMm * row,
  );
  return (
    <div className="heightmap-values-wrap">
      <div className="heightmap-values-summary">
        <span>MIN <code>{values.length ? minimum.toFixed(3) : "—"}</code></span>
        <span>MAX <code>{values.length ? maximum.toFixed(3) : "—"}</code></span>
        <span>Δ <code>{values.length ? (maximum - minimum).toFixed(3) : "—"}</code> mm</span>
      </div>
      <div className="heightmap-values-scroll">
        <table aria-label="Числовая карта высот">
          <thead>
            <tr><th>Y \ X</th>{xCoordinates.map((x, column) => <th key={column}>{x.toFixed(2)}</th>)}</tr>
          </thead>
          <tbody>
            {matrix.slice().reverse().map((row, reverseRow) => (
              <tr key={matrix.length - reverseRow - 1}>
                <th>{yCoordinates[matrix.length - reverseRow - 1].toFixed(2)}</th>
                {row.map((value, column) => (
                  <td key={column} style={value === undefined ? undefined : { borderColor: heightColor(value, minimum, maximum) }}>
                    <strong>{value === undefined ? "—" : value.toFixed(3)}</strong>
                    <small>{value === undefined ? "" : `${value - reference >= 0 ? "+" : ""}${(value - reference).toFixed(3)}`}</small>
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
