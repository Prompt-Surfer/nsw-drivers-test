import requests
import json
import time
import os
from dotenv import load_dotenv
load_dotenv()

with open('../data/centers.json', 'r') as file:
    centers = json.load(file)

geocode_url = "https://maps.googleapis.com/maps/api/geocode/json"

for center in centers:
    address = f"{center['name']} NSW Service Center, New South Wales, Australia"
    
    params = {
        "address": address,
        "key": os.getenv('GEOCODE')
    }
    
    response = requests.get(geocode_url, params=params)
    results = response.json()
    
    if results['status'] == 'OK':
        location = results['results'][0]['geometry']['location']
        center['latitude'] = location['lat']
        center['longitude'] = location['lng']
        print(f"Updated coordinates for {center['name']}")
    else:
        print(f"Failed to get coordinates for {center['name']}: {results['status']}")
    
    time.sleep(0.2)

with open('../data/updated_centers.json', 'w') as file:
    json.dump(centers, file, indent=2)
