import sys
import time
import json
from datetime import datetime
from selenium import webdriver
from selenium.webdriver.common.by import By
from selenium.webdriver.support.ui import Select
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.chrome.service import Service


chrome_options = Options()
chrome_options.add_argument("--no-sandbox")
chrome_options.add_argument("--disable-dev-shm-usage")
chrome_options.add_argument("user-agent=Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/103.0.5060.114 Safari/537.36")
chrome_options.add_argument("--disable-blink-features=AutomationControlled") 
chrome_options.add_experimental_option("excludeSwitches", ["enable-automation"]) 
chrome_options.add_experimental_option("useAutomationExtension", False) 
chrome_options.add_argument('--log-level=3')
chrome_options.set_capability('goog:loggingPrefs', {'browser': 'ALL', 'driver': 'ALL'})

settings = {'wait_timer': 2, 'wait_timer_car': 15, 'username': 24308671, 'password': 'DhJRU8Bm', 'have_booking': True}

service = Service(log_path="chromedriver.log", service_args=['--verbose'])
driver = webdriver.Chrome(options=chrome_options, service=service)
driver.execute_script("Object.defineProperty(navigator, 'webdriver', {get: () => undefined})") 
driver.get("https://www.myrta.com/wps/portal/extvp/myrta/login/")
driver.find_element(By.ID,"widget_cardNumber").send_keys(settings['username'])
driver.find_element(By.ID,"widget_password").send_keys(settings['password'])
time.sleep(settings['wait_timer'])
driver.find_element(By.ID,"nextButton").click()
if(settings['have_booking']):
    driver.find_element(By.XPATH,'//*[text()="Manage booking"]').click()
    driver.find_element(By.ID,"changeLocationButton").click()
    time.sleep(settings['wait_timer'])
else:
    driver.find_element(By.XPATH,'//*[text()="Book test"]').click()
    driver.find_element(By.ID,"CAR").click()
    time.sleep(settings['wait_timer_car'])
    driver.find_element(By.XPATH,"//fieldset[@id='DC']/span[contains(@class, 'rms_testItemResult')]").click()
    time.sleep(settings['wait_timer'])
    driver.find_element(By.ID,"nextButton").click()
    time.sleep(settings['wait_timer'])
    driver.find_element(By.ID,"checkTerms").click()
    time.sleep(settings['wait_timer'])
    driver.find_element(By.ID,"nextButton").click()
    time.sleep(settings['wait_timer'])
    driver.find_element(By.ID,"rms_batLocLocSel").click()
    time.sleep(settings['wait_timer'])
driver.find_element(By.ID,"rms_batLocLocSel").click()
time.sleep(settings['wait_timer'])
select_box = driver.find_element(By.ID,"rms_batLocationSelect2")
Select(select_box).select_by_value("63")
time.sleep(settings['wait_timer'])
driver.find_element(By.ID,"nextButton").click()
if(driver.find_element(By.ID,"getEarliestTime").size!=0):
    if(driver.find_element(By.ID,"getEarliestTime").is_displayed()):
        if(driver.find_element(By.ID,"getEarliestTime").is_enabled()):
            driver.find_element(By.ID,"getEarliestTime").click()
result = driver.execute_script('return timeslots')
print('{"location":"'+'","result":'+json.dumps(result)+'}\n')
results_file.close()
driver.quit()
