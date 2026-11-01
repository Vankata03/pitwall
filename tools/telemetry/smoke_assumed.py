#!/usr/bin/env python3
"""Generate assumed full-grid wire inputs, never captured telemetry or a golden.

Writes PWCAP for conversion with the public pitrec tool. All names are invented.
24 cars, three laps, telemetry at 10 Hz; car 23 has restricted telemetry.
"""
import json
import math
from pathlib import Path
import struct
import sys
from assumed_datagram import build


def generate(path):
    meta = json.dumps({'label': 'assumed-full-grid', 'started_utc': '2026-01-01T00:00:00+00:00'}).encode()
    with Path(path).open('xb') as out:
        out.write(b'PWCAP1\n' + struct.pack('<I', len(meta)) + meta)
        for tick in range(301):
            t = tick / 10
            common = {'header.session_uid': 51001, 'header.session_time': t,
                      'header.frame_identifier': tick, 'header.overall_frame_identifier': tick}
            packets = {
                1: {'track_id': 10, 'track_length': 7007, 'session_type': 10,
                    'total_laps': 10, 'track_temperature': 32, 'air_temperature': 22,
                    'session_duration': 900, 'session_time_left': 900 - int(t),
                    'sector2_lap_distance_start': 2300, 'sector3_lap_distance_start': 4600},
                4: {'num_active_cars': 24}, 0: {}, 2: {}, 6: {}, 7: {}, 10: {},
            }
            for car in range(24):
                def fields(pid, group, **values):
                    packets[pid].update({f'{group}.{car}.{k}': v for k, v in values.items()})
                fields(4, 'participants', ai_controlled=1, driver_id=car, team_id=car % 10,
                       race_number=car + 1, your_telemetry=0 if car == 23 else 1,
                       show_online_names=1)
                for i, b in enumerate(f'Assumed AI {car+1}'.encode()):
                    packets[4][f'participants.{car}.name.{i}'] = b
                distance = (t % 10) / 10 * 7007
                angle = distance / 7007 * 2 * math.pi + car * .01
                fields(0, 'car_motion_data', world_position_x=math.cos(angle)*1000,
                       world_position_z=math.sin(angle)*1000, g_force_lateral=800,
                       g_force_longitudinal=300)
                fields(2, 'lap_data', car_position=car+1, grid_position=car+1,
                       current_lap_num=int(t // 10)+1, current_lap_time_in_ms=int(t%10*1000),
                       last_lap_time_in_ms=10000 if t >= 10 else 0, lap_distance=distance,
                       total_distance=t/10*7007, sector=int(t%10//3.34),
                       driver_status=1, result_status=2, delta_to_race_leader_ms_part=car*100,
                       delta_to_car_in_front_ms_part=100)
                fields(6, 'car_telemetry_data', speed=200+tick%80, throttle=.8, brake=.1,
                       gear=6, engine_rpm=11000, engine_temperature=100)
                fields(7, 'car_status_data', fuel_in_tank=40-t/10, fuel_capacity=110,
                       fuel_remaining_laps=2, max_rpm=13000, idle_rpm=4000, max_gears=8,
                       actual_tyre_compound=18, visual_tyre_compound=18, tyres_age_laps=int(t//10),
                       ers_store_energy=3000000, ers_deploy_mode=1)
                for wheel in range(4):
                    for key, value in [('brakes_temperature', 500), ('tyres_surface_temperature', 90),
                                       ('tyres_inner_temperature', 85), ('tyres_pressure', 23.5)]:
                        packets[6][f'car_telemetry_data.{car}.{key}.{wheel}'] = value
                    packets[10][f'car_damage_data.{car}.tyres_wear.{wheel}'] = 10+t/10
            for pid, values in packets.items():
                data = build(2026, pid, common | values)
                out.write(struct.pack('<QH', tick*100_000_000, len(data)) + data)


if __name__ == '__main__':
    generate(sys.argv[1])
