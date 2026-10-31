import { useEffect, useMemo, useState } from 'react';
import type { ParticipantData } from '../../generated/wire/ParticipantData';
import { chooseLivery, type LiveryCar } from '../../liveries/choose';
import { fallbackCarSvg } from '../../liveries/fallbackCar';

/** Pack image names from the host (`GET /api/liveries`); empty when the host cannot say. */
function useLiveryFiles(): readonly string[] {
  const [files, setFiles] = useState<readonly string[]>([]);
  useEffect(() => {
    let live = true;
    fetch('/api/liveries')
      .then((response) => response.json() as Promise<{ files?: unknown }>)
      .then((body) => {
        if (live && Array.isArray(body.files)) setFiles(body.files.filter((f): f is string => typeof f === 'string'));
      })
      .catch(() => {});
    return () => {
      live = false;
    };
  }, []);
  return files;
}

export function liveryCarOf(participant: ParticipantData | undefined): LiveryCar | undefined {
  if (!participant) return undefined;
  return {
    teamId: participant.team_id,
    myTeam: participant.my_team,
    numColours: participant.num_colours,
    colours: participant.livery_colour.map((c) => ({ r: c.red, g: c.green, b: c.blue })),
  };
}

/**
 * The car at the centre of the tile: the user's pack image for the team, else the original
 * fallback car in the game's livery colours. Decorative: it defines no anchors and never
 * implies a value, so it has no text alternative.
 */
export function CarImage({ participant }: { participant: ParticipantData | undefined }) {
  const files = useLiveryFiles();
  const choice = chooseLivery(liveryCarOf(participant), files);
  const [broken, setBroken] = useState<string | null>(null);
  const packUrl = choice.source === 'pack' && broken !== choice.url ? choice.url : null;
  const coloursKey = JSON.stringify(choice.colours);
  const fallback = useMemo(
    () => `data:image/svg+xml;charset=utf-8,${encodeURIComponent(fallbackCarSvg(choice.colours))}`,
    // `choice.colours` is rebuilt on every call; its content is what matters.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [coloursKey],
  );
  return (
    <img
      className="pw-cs-car"
      data-testid="car-image"
      src={packUrl ?? fallback}
      alt=""
      draggable={false}
      onError={() => {
        if (packUrl) setBroken(packUrl);
      }}
    />
  );
}
