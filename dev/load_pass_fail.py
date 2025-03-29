import os
import csv
import json
import time
import math
import random
import requests
import urllib.parse
from typing import Dict, List, Set, Optional, Tuple
from pathlib import Path

GEOCODING_CACHE: Dict[str, Dict] = {}
CACHE_FILE = "geocoding_cache.json"

def load_cache():
    global GEOCODING_CACHE
    if os.path.exists(CACHE_FILE):
        try:
            with open(CACHE_FILE, 'r') as f:
                GEOCODING_CACHE = json.load(f)
            print(f"Loaded {len(GEOCODING_CACHE)} cached geocoding results")
        except Exception as e:
            print(f"Error loading cache: {e}")
            GEOCODING_CACHE = {}


def save_cache():
    with open(CACHE_FILE, 'w') as f:
        json.dump(GEOCODING_CACHE, f)
    print(f"Saved {len(GEOCODING_CACHE)} geocoding results to cache")


def geocode_address(address: str) -> Dict:
    global GEOCODING_CACHE
    
    
    if address in GEOCODING_CACHE:
        return GEOCODING_CACHE[address]
    
    
    time.sleep(1)
    
    encoded_address = urllib.parse.quote(address)
    url = f"https://nominatim.openstreetmap.org/search?q={encoded_address}&format=json&limit=1&addressdetails=1&countrycodes=au"
    
    headers = {"User-Agent": "NSW Drivers Test Nearest Date - teegee567/1.0"}
    
    try:
        response = requests.get(url, headers=headers)
        response.raise_for_status()
        
        results = response.json()
        
        if not results:
            print(f"No geocoding results for: {address}")
            return None
        
        result = results[0]
        
        geocoding_result = {
            "latitude": float(result.get("lat", 0)),
            "longitude": float(result.get("lon", 0)),
            "display_name": result.get("display_name", "")
        }
        
        
        GEOCODING_CACHE[address] = geocoding_result
        return geocoding_result
    
    except Exception as e:
        print(f"Error geocoding {address}: {e}")
        return None


def haversine_distance(lat1: float, lon1: float, lat2: float, lon2: float) -> float:
    R = 6371.0  
    
    dlat = math.radians(lat2 - lat1)
    dlon = math.radians(lon2 - lon1)
    
    a = (math.sin(dlat/2)**2) + math.cos(math.radians(lat1)) * math.cos(math.radians(lat2)) * (math.sin(dlon/2)**2)
    c = 2 * math.atan2(math.sqrt(a), math.sqrt(1-a))
    
    return R * c  


def calculate_weight(distance: float, max_distance: float = 50.0) -> float:
    
    if distance > max_distance:
        return 0
    
    
    
    weight = 1.0 / (distance + 0.5)**2
    
    return weight


def choose_center_weighted(coords: Dict, centers: List[Dict], max_centers: int = 3) -> Dict:
    center_weights = []
    for center in centers:
        dist = haversine_distance(
            coords['latitude'], coords['longitude'],
            center['latitude'], center['longitude']
        )
        weight = calculate_weight(dist)
        if weight > 0:
            center_weights.append({
                'center': center,
                'distance': dist,
                'weight': weight
            })
    
    
    center_weights.sort(key=lambda x: x['weight'], reverse=True)
    center_weights = center_weights[:max_centers]
    
    if not center_weights:
        return None
    
    total_weight = sum(cw['weight'] for cw in center_weights)
    if total_weight > 0:
        for cw in center_weights:
            cw['weight'] /= total_weight
    
    
    weights = [cw['weight'] for cw in center_weights]
    center_idx = random.choices(
        range(len(center_weights)), 
        weights=weights, 
        k=1
    )[0]
    
    chosen_center = center_weights[center_idx]['center']
    return chosen_center

def process_csv_file(csv_path: str, centers: List[Dict]) -> Dict[int, Dict]:
    print(f"Processing file: {csv_path}")
    center_stats = {center['id']: {'passes': 0, 'failures': 0} for center in centers}

    unique_lgas: Set[str] = set()
    records = []
    with open(csv_path, 'r', encoding='utf-8') as f:
        reader = csv.DictReader(f, delimiter='|')
        for row in reader:
            
            if row.get('LICENCE TEST REPORTING CATEGORY') == 'C Class Driving Test':
                lga = row.get('CUSTOMER ADDRESS LGA', '').strip()
                if lga:
                    unique_lgas.add(lga)
                    records.append(row)
    
    lga_coordinates = {}
    for lga in unique_lgas:
        coords = geocode_address(lga)
        if coords:
            lga_coordinates[lga] = coords
    
    for record in records:
        lga = record.get('CUSTOMER ADDRESS LGA', '').strip()
        result = record.get('RESULT', '')
        count_str = record.get('COUNT', '0')
        
        if count_str == '<=5':
            count = 3
        else:
            try:
                count = int(count_str)
            except ValueError:
                count = 0
        
        if lga not in lga_coordinates:
            continue
        
        coords = lga_coordinates[lga]
        
        chosen_center = choose_center_weighted(coords, centers)
        if chosen_center:
            if result == 'Pass':
                center_stats[chosen_center['id']]['passes'] += count
            elif result == 'Fail':
                center_stats[chosen_center['id']]['failures'] += count
    
    return center_stats

def analyze_driving_test_data(data_dir: str, centers_path: str, output_path: str):
    print("Starting analysis of driving test data...")
    
    load_cache()
    
    with open(centers_path, 'r') as f:
        centers = json.load(f)
    
    for center in centers:
        center['passes'] = 0
        center['failures'] = 0
        center['pass_rate'] = 0.0
    
    csv_files = [f for f in os.listdir(data_dir) if f.endswith('.csv')]
    
    if not csv_files:
        print(f"No CSV files found in {data_dir}")
        return
    
    for csv_file in csv_files:
        file_path = os.path.join(data_dir, csv_file)
        center_stats = process_csv_file(file_path, centers)
        
        for center in centers:
            center_id = center['id']
            if center_id in center_stats:
                center['passes'] += center_stats[center_id]['passes']
                center['failures'] += center_stats[center_id]['failures']

    for center in centers:
        total = center['passes'] + center['failures']
        if total > 0:
            center['pass_rate'] = (center['passes'] / total) * 100.0
    
    with open(output_path, 'w') as f:
        json.dump(centers, f, indent=2)
    
    print(f"Analysis complete! Results saved to {output_path}")
    
    save_cache()

if __name__ == '__main__':
    data_directory = "temp"  
    centers_file = "data/centers.json"  
    output_file = "test_centers_with_stats.json"  
    
    analyze_driving_test_data(data_directory, centers_file, output_file)
