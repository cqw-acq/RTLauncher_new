export const dynamic = "force-static";
export const dynamicParams = false;

export async function GET() {
  return Response.json({ ok: true, sources: { modrinth: { ok: true, count: 0 }, curseforge: { ok: true, count: 0 } } });
}